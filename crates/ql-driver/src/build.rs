use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::analyze_source;
use ql_codegen_llvm::{CodegenInput, CodegenMode, emit_module};
use ql_diagnostics::{Diagnostic, Label};
use ql_runtime::{RuntimeCapability, collect_runtime_hook_signatures};

use crate::ffi::{
    CHeaderArtifact, CHeaderError, CHeaderOptions, CHeaderSurface, emit_c_header_from_analysis,
    exported_c_symbol_names,
};
use crate::toolchain::{ToolchainError, ToolchainOptions, discover_toolchain};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildEmit {
    LlvmIr,
    Object,
    Executable,
    DynamicLibrary,
    StaticLibrary,
}

impl BuildEmit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlvmIr => "llvm-ir",
            Self::Object => "object",
            Self::Executable => "executable",
            Self::DynamicLibrary => "dylib",
            Self::StaticLibrary => "staticlib",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildOptions {
    pub emit: BuildEmit,
    pub profile: BuildProfile,
    pub output: Option<PathBuf>,
    pub c_header: Option<BuildCHeaderOptions>,
    pub toolchain: ToolchainOptions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildCHeaderOptions {
    pub output: Option<PathBuf>,
    pub surface: CHeaderSurface,
}

impl Default for BuildEmit {
    fn default() -> Self {
        Self::LlvmIr
    }
}

impl Default for BuildProfile {
    fn default() -> Self {
        Self::Debug
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildArtifact {
    pub emit: BuildEmit,
    pub profile: BuildProfile,
    pub path: PathBuf,
    pub c_header: Option<CHeaderArtifact>,
}

#[derive(Debug)]
pub enum BuildError {
    InvalidInput(String),
    Io {
        path: PathBuf,
        error: io::Error,
    },
    Diagnostics {
        path: PathBuf,
        source: String,
        diagnostics: Vec<Diagnostic>,
    },
    Toolchain {
        error: ToolchainError,
        preserved_artifacts: Vec<PathBuf>,
    },
}

impl BuildError {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::InvalidInput(_) | Self::Toolchain { .. } => None,
            Self::Io { path, .. } | Self::Diagnostics { path, .. } => Some(path),
        }
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            Self::Diagnostics { source, .. } => Some(source),
            Self::InvalidInput(_) | Self::Io { .. } | Self::Toolchain { .. } => None,
        }
    }

    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        match self {
            Self::Diagnostics { diagnostics, .. } => Some(diagnostics),
            Self::InvalidInput(_) | Self::Io { .. } | Self::Toolchain { .. } => None,
        }
    }

    pub fn toolchain_error(&self) -> Option<&ToolchainError> {
        match self {
            Self::Toolchain { error, .. } => Some(error),
            Self::InvalidInput(_) | Self::Io { .. } | Self::Diagnostics { .. } => None,
        }
    }

    pub fn preserved_artifacts(&self) -> Option<&[PathBuf]> {
        match self {
            Self::Toolchain {
                preserved_artifacts,
                ..
            } if !preserved_artifacts.is_empty() => Some(preserved_artifacts),
            Self::InvalidInput(_)
            | Self::Io { .. }
            | Self::Diagnostics { .. }
            | Self::Toolchain { .. } => None,
        }
    }

    pub fn intermediate_ir(&self) -> Option<&Path> {
        self.preserved_artifacts()?
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen.ll"))
            })
            .map(PathBuf::as_path)
    }
}

pub fn build_file(path: &Path, options: &BuildOptions) -> Result<BuildArtifact, BuildError> {
    if !path.is_file() {
        return Err(BuildError::InvalidInput(format!(
            "`{}` is not a file",
            path.display()
        )));
    }

    if options.c_header.is_some() && !build_emit_supports_c_header(options.emit) {
        return Err(BuildError::InvalidInput(format!(
            "build-side C header generation only supports `dylib` and `staticlib`, found `{}`",
            options.emit.as_str()
        )));
    }

    let source = fs::read_to_string(path).map_err(|error| BuildError::Io {
        path: path.to_path_buf(),
        error,
    })?;

    let analysis = analyze_source(&source).map_err(|diagnostics| BuildError::Diagnostics {
        path: path.to_path_buf(),
        source: source.clone(),
        diagnostics,
    })?;

    if analysis.has_errors() {
        return Err(BuildError::Diagnostics {
            path: path.to_path_buf(),
            source: source.clone(),
            diagnostics: analysis.diagnostics().to_vec(),
        });
    }

    let exported_symbols = if options.emit == BuildEmit::DynamicLibrary {
        let symbols = exported_c_symbol_names(analysis.hir());
        if symbols.is_empty() {
            return Err(BuildError::InvalidInput(
                "dynamic library emission currently requires at least one public top-level `extern \"c\"` function definition"
                    .to_owned(),
            ));
        }
        symbols
    } else {
        Vec::new()
    };

    let runtime_diagnostics = runtime_requirement_diagnostics(&analysis, options.emit);
    let mut runtime_capabilities = analysis
        .runtime_requirements()
        .iter()
        .map(|requirement| requirement.capability)
        .collect::<Vec<_>>();
    if options.emit == BuildEmit::Executable
        && analysis
            .hir()
            .items
            .iter()
            .filter_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ql_hir::ItemKind::Function(function) => Some(function),
                _ => None,
            })
            .any(|function| function.name == "main" && function.is_async)
    {
        if !runtime_capabilities.contains(&RuntimeCapability::TaskSpawn) {
            runtime_capabilities.push(RuntimeCapability::TaskSpawn);
        }
        if !runtime_capabilities.contains(&RuntimeCapability::TaskAwait) {
            runtime_capabilities.push(RuntimeCapability::TaskAwait);
        }
    }
    let runtime_hooks = collect_runtime_hook_signatures(runtime_capabilities.iter().copied());
    let module_name = default_module_name(path);
    let ir = match emit_module(CodegenInput {
        module_name: &module_name,
        mode: codegen_mode(options.emit),
        hir: analysis.hir(),
        mir: analysis.mir(),
        resolution: analysis.resolution(),
        typeck: analysis.typeck(),
        runtime_hooks: &runtime_hooks,
    }) {
        Ok(ir) => {
            if !runtime_diagnostics.is_empty() {
                return Err(BuildError::Diagnostics {
                    path: path.to_path_buf(),
                    source: source.clone(),
                    diagnostics: runtime_diagnostics,
                });
            }
            ir
        }
        Err(error) => {
            return Err(BuildError::Diagnostics {
                path: path.to_path_buf(),
                source: source.clone(),
                diagnostics: merge_unique_diagnostics(
                    error.into_diagnostics(),
                    &runtime_diagnostics,
                ),
            });
        }
    };

    let output_path = match &options.output {
        Some(path) => path.clone(),
        None => {
            let build_root = env::current_dir().map_err(|error| BuildError::Io {
                path: PathBuf::from("."),
                error,
            })?;
            default_output_path(&build_root, path, options.profile, options.emit)
        }
    };
    let c_header_options =
        resolve_build_c_header_options(path, &output_path, options.c_header.as_ref());
    if let Some(header_options) = c_header_options.as_ref() {
        let header_path = header_options
            .output
            .as_ref()
            .expect("build-side C header output path should be resolved");
        if header_path == &output_path {
            return Err(BuildError::InvalidInput(format!(
                "build-side C header output `{}` must differ from the primary artifact output",
                header_path.display()
            )));
        }
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| BuildError::Io {
            path: parent.to_path_buf(),
            error,
        })?;
    }

    match options.emit {
        BuildEmit::LlvmIr => {
            fs::write(&output_path, ir).map_err(|error| BuildError::Io {
                path: output_path.clone(),
                error,
            })?;
        }
        BuildEmit::Object => {
            build_object_file(&output_path, &ir, &options.toolchain)?;
        }
        BuildEmit::Executable => {
            build_executable_file(&output_path, &ir, &options.toolchain)?;
        }
        BuildEmit::DynamicLibrary => {
            build_dynamic_library_file(&output_path, &ir, &exported_symbols, &options.toolchain)?;
        }
        BuildEmit::StaticLibrary => {
            build_static_library_file(&output_path, &ir, &options.toolchain)?;
        }
    }

    let c_header = match c_header_options {
        Some(ref header_options) => {
            let header_path = header_options
                .output
                .clone()
                .expect("build-side C header output path should be resolved");
            match emit_c_header_from_analysis(path, &source, &analysis, header_options) {
                Ok(artifact) => Some(artifact),
                Err(error) => {
                    cleanup_artifacts(&[output_path.clone(), header_path]);
                    return Err(map_c_header_error(error));
                }
            }
        }
        None => None,
    };

    Ok(BuildArtifact {
        emit: options.emit,
        profile: options.profile,
        path: output_path,
        c_header,
    })
}

fn runtime_requirement_diagnostics(
    analysis: &ql_analysis::Analysis,
    emit: BuildEmit,
) -> Vec<Diagnostic> {
    analysis
        .runtime_requirements()
        .iter()
        .filter_map(|requirement| runtime_requirement_diagnostic(requirement, emit))
        .collect()
}

fn runtime_requirement_diagnostic(
    requirement: &ql_analysis::RuntimeRequirement,
    emit: BuildEmit,
) -> Option<Diagnostic> {
    runtime_requirement_message(requirement.capability, emit)
        .map(|message| Diagnostic::error(message).with_label(Label::new(requirement.span)))
}

fn runtime_requirement_message(
    capability: RuntimeCapability,
    emit: BuildEmit,
) -> Option<&'static str> {
    match capability {
        // All async capabilities are now open for staticlib, dylib, and executable builds.
        // LlvmIr and Object emits remain conservatively blocked (no entry lifecycle lowering).
        RuntimeCapability::AsyncFunctionBodies
        | RuntimeCapability::TaskAwait
        | RuntimeCapability::TaskSpawn
            if matches!(
                emit,
                BuildEmit::StaticLibrary | BuildEmit::DynamicLibrary | BuildEmit::Executable
            ) =>
        {
            None
        }
        RuntimeCapability::AsyncIteration
            if matches!(
                emit,
                BuildEmit::StaticLibrary | BuildEmit::DynamicLibrary | BuildEmit::Executable
            ) =>
        {
            None
        }
        RuntimeCapability::AsyncFunctionBodies => {
            Some("LLVM IR backend foundation does not support `async fn` yet")
        }
        RuntimeCapability::TaskSpawn => {
            Some("LLVM IR backend foundation does not support `spawn` yet")
        }
        RuntimeCapability::TaskAwait => {
            Some("LLVM IR backend foundation does not support `await` yet")
        }
        RuntimeCapability::AsyncIteration => {
            Some("LLVM IR backend foundation does not support `for await` lowering yet")
        }
    }
}

fn merge_unique_diagnostics(
    mut diagnostics: Vec<Diagnostic>,
    additions: &[Diagnostic],
) -> Vec<Diagnostic> {
    for diagnostic in additions {
        if runtime_operator_message(diagnostic.message.as_str())
            && diagnostics
                .iter()
                .any(|existing| existing.message == diagnostic.message)
        {
            continue;
        }
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics
}

fn runtime_operator_message(message: &str) -> bool {
    matches!(
        message,
        "LLVM IR backend foundation does not support `await` yet"
            | "LLVM IR backend foundation does not support `spawn` yet"
            | "LLVM IR backend foundation does not support `for await` lowering yet"
    )
}

fn build_emit_supports_c_header(emit: BuildEmit) -> bool {
    matches!(emit, BuildEmit::DynamicLibrary | BuildEmit::StaticLibrary)
}

fn resolve_build_c_header_options(
    input_path: &Path,
    artifact_path: &Path,
    options: Option<&BuildCHeaderOptions>,
) -> Option<CHeaderOptions> {
    options.map(|options| {
        let output = options.output.clone().unwrap_or_else(|| {
            default_build_c_header_output_path(artifact_path, input_path, options.surface)
        });
        CHeaderOptions {
            output: Some(output),
            surface: options.surface,
        }
    })
}

fn default_build_c_header_output_path(
    artifact_path: &Path,
    input_path: &Path,
    surface: CHeaderSurface,
) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let file_name = match surface.output_suffix() {
        Some(suffix) => format!("{stem}.{suffix}.h"),
        None => format!("{stem}.h"),
    };

    let directory = artifact_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    directory.join(file_name)
}

fn map_c_header_error(error: CHeaderError) -> BuildError {
    match error {
        CHeaderError::InvalidInput(message) => BuildError::InvalidInput(message),
        CHeaderError::Io { path, error } => BuildError::Io { path, error },
        CHeaderError::Diagnostics {
            path,
            source,
            diagnostics,
        } => BuildError::Diagnostics {
            path,
            source,
            diagnostics,
        },
    }
}

pub fn default_output_path(
    build_root: &Path,
    input_path: &Path,
    profile: BuildProfile,
    emit: BuildEmit,
) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    build_root
        .join("target")
        .join("ql")
        .join(profile.dir_name())
        .join(default_output_name(stem, emit))
}

fn build_object_file(
    output_path: &Path,
    ir: &str,
    toolchain_options: &ToolchainOptions,
) -> Result<(), BuildError> {
    let intermediate_ir = intermediate_ir_path(output_path);
    fs::write(&intermediate_ir, ir).map_err(|error| BuildError::Io {
        path: intermediate_ir.clone(),
        error,
    })?;

    let toolchain = discover_toolchain(toolchain_options)
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;

    if let Err(error) = toolchain.compile_llvm_ir_to_object(&intermediate_ir, output_path) {
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    cleanup_artifacts(&[intermediate_ir]);
    Ok(())
}

fn build_executable_file(
    output_path: &Path,
    ir: &str,
    toolchain_options: &ToolchainOptions,
) -> Result<(), BuildError> {
    let intermediate_ir = intermediate_ir_path(output_path);
    fs::write(&intermediate_ir, ir).map_err(|error| BuildError::Io {
        path: intermediate_ir.clone(),
        error,
    })?;

    let toolchain = discover_toolchain(toolchain_options)
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;
    let intermediate_object = intermediate_object_path(output_path);

    if let Err(error) = toolchain.compile_llvm_ir_to_object(&intermediate_ir, &intermediate_object)
    {
        let _ = fs::remove_file(&intermediate_object);
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    if let Err(error) = toolchain.link_object_to_executable(&intermediate_object, output_path) {
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(
            error,
            vec![intermediate_ir, intermediate_object],
        ));
    }

    cleanup_artifacts(&[intermediate_ir, intermediate_object]);
    Ok(())
}

fn build_static_library_file(
    output_path: &Path,
    ir: &str,
    toolchain_options: &ToolchainOptions,
) -> Result<(), BuildError> {
    let intermediate_ir = intermediate_ir_path(output_path);
    fs::write(&intermediate_ir, ir).map_err(|error| BuildError::Io {
        path: intermediate_ir.clone(),
        error,
    })?;

    let toolchain = discover_toolchain(toolchain_options)
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;
    toolchain
        .ensure_archiver_available()
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;
    let intermediate_object = intermediate_object_path(output_path);

    if let Err(error) = toolchain.compile_llvm_ir_to_object(&intermediate_ir, &intermediate_object)
    {
        let _ = fs::remove_file(&intermediate_object);
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    if let Err(error) =
        toolchain.archive_object_to_static_library(&intermediate_object, output_path)
    {
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(
            error,
            vec![intermediate_ir, intermediate_object],
        ));
    }

    cleanup_artifacts(&[intermediate_ir, intermediate_object]);
    Ok(())
}

fn build_dynamic_library_file(
    output_path: &Path,
    ir: &str,
    exported_symbols: &[String],
    toolchain_options: &ToolchainOptions,
) -> Result<(), BuildError> {
    let intermediate_ir = intermediate_ir_path(output_path);
    fs::write(&intermediate_ir, ir).map_err(|error| BuildError::Io {
        path: intermediate_ir.clone(),
        error,
    })?;

    let toolchain = discover_toolchain(toolchain_options)
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;
    let intermediate_object = intermediate_object_path(output_path);

    if let Err(error) = toolchain.compile_llvm_ir_to_object(&intermediate_ir, &intermediate_object)
    {
        let _ = fs::remove_file(&intermediate_object);
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    if let Err(error) = toolchain.link_object_to_dynamic_library(
        &intermediate_object,
        output_path,
        exported_symbols,
    ) {
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(
            error,
            vec![intermediate_ir, intermediate_object],
        ));
    }

    cleanup_artifacts(&[intermediate_ir, intermediate_object]);
    Ok(())
}

fn intermediate_ir_path(output_path: &Path) -> PathBuf {
    intermediate_artifact_path(output_path, "ll")
}

fn intermediate_object_path(output_path: &Path) -> PathBuf {
    intermediate_artifact_path(output_path, object_extension())
}

fn intermediate_artifact_path(output_path: &Path, extension: &str) -> PathBuf {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    parent.join(format!("{stem}.{unique}.codegen.{extension}"))
}

fn cleanup_artifacts(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn toolchain_failure(error: ToolchainError, preserved_artifacts: Vec<PathBuf>) -> BuildError {
    BuildError::Toolchain {
        error,
        preserved_artifacts,
    }
}

fn default_module_name(path: &Path) -> String {
    let raw = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    sanitize_symbol(raw)
}

fn sanitize_symbol(raw: &str) -> String {
    let mut output = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        "module".to_owned()
    } else {
        output
    }
}

fn object_extension() -> &'static str {
    if cfg!(windows) { "obj" } else { "o" }
}

fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

fn static_library_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.lib")
    } else {
        format!("lib{stem}.a")
    }
}

fn dynamic_library_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

fn default_output_name(stem: &str, emit: BuildEmit) -> String {
    match emit {
        BuildEmit::LlvmIr => format!("{stem}.ll"),
        BuildEmit::Object => format!("{stem}.{}", object_extension()),
        BuildEmit::Executable => executable_name(stem),
        BuildEmit::DynamicLibrary => dynamic_library_name(stem),
        BuildEmit::StaticLibrary => static_library_name(stem),
    }
}

fn codegen_mode(emit: BuildEmit) -> CodegenMode {
    match emit {
        BuildEmit::LlvmIr | BuildEmit::Object | BuildEmit::Executable => CodegenMode::Program,
        BuildEmit::DynamicLibrary | BuildEmit::StaticLibrary => CodegenMode::Library,
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::toolchain::{
        ArchiverFlavor, ArchiverInvocation, ProgramInvocation, ToolchainOptions,
    };

    use super::{
        BuildCHeaderOptions, BuildEmit, BuildError, BuildOptions, BuildProfile, CHeaderSurface,
        build_file, default_build_c_header_output_path, default_output_path,
    };

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create test parent directory");
            }
            fs::write(&path, contents).expect("write test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn default_output_path_uses_target_ql_layout() {
        let llvm_ir = default_output_path(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            BuildProfile::Release,
            BuildEmit::LlvmIr,
        );
        let object = default_output_path(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            BuildProfile::Release,
            BuildEmit::Object,
        );
        let executable = default_output_path(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            BuildProfile::Release,
            BuildEmit::Executable,
        );
        let dynamic_library = default_output_path(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            BuildProfile::Release,
            BuildEmit::DynamicLibrary,
        );
        let static_library = default_output_path(
            Path::new("D:/workspace/demo"),
            Path::new("src/app.ql"),
            BuildProfile::Release,
            BuildEmit::StaticLibrary,
        );

        assert_eq!(
            llvm_ir,
            PathBuf::from("D:/workspace/demo/target/ql/release/app.ll")
        );
        assert_eq!(
            object,
            PathBuf::from(format!(
                "D:/workspace/demo/target/ql/release/app.{}",
                if cfg!(windows) { "obj" } else { "o" }
            ))
        );
        assert_eq!(
            executable,
            PathBuf::from(if cfg!(windows) {
                "D:/workspace/demo/target/ql/release/app.exe"
            } else {
                "D:/workspace/demo/target/ql/release/app"
            })
        );
        assert_eq!(
            dynamic_library,
            PathBuf::from(if cfg!(windows) {
                "D:/workspace/demo/target/ql/release/app.dll"
            } else if cfg!(target_os = "macos") {
                "D:/workspace/demo/target/ql/release/libapp.dylib"
            } else {
                "D:/workspace/demo/target/ql/release/libapp.so"
            })
        );
        assert_eq!(
            static_library,
            PathBuf::from(if cfg!(windows) {
                "D:/workspace/demo/target/ql/release/app.lib"
            } else {
                "D:/workspace/demo/target/ql/release/libapp.a"
            })
        );
    }

    #[test]
    fn default_build_c_header_output_path_uses_artifact_directory_and_source_stem() {
        let exports = default_build_c_header_output_path(
            Path::new("D:/workspace/demo/target/ql/debug/libffi_export.so"),
            Path::new("src/ffi_export.ql"),
            CHeaderSurface::Exports,
        );
        let imports = default_build_c_header_output_path(
            Path::new("D:/workspace/demo/artifacts/math.lib"),
            Path::new("pkg/math.ql"),
            CHeaderSurface::Imports,
        );
        let both = default_build_c_header_output_path(
            Path::new("D:/workspace/demo/artifacts/libsurface.a"),
            Path::new("pkg/surface.ql"),
            CHeaderSurface::Both,
        );

        assert_eq!(
            exports,
            PathBuf::from("D:/workspace/demo/target/ql/debug/ffi_export.h")
        );
        assert_eq!(
            imports,
            PathBuf::from("D:/workspace/demo/artifacts/math.imports.h")
        );
        assert_eq!(
            both,
            PathBuf::from("D:/workspace/demo/artifacts/surface.ffi.h")
        );
    }

    #[test]
    fn build_file_writes_llvm_ir_to_explicit_output() {
        let dir = TestDir::new("ql-driver-build");
        let source = dir.write(
            "sample.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    return add_one(41)
}
"#,
        );
        let output = dir.path().join("artifacts/sample.ll");
        let options = BuildOptions {
            emit: BuildEmit::LlvmIr,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions::default(),
        };

        let artifact = build_file(&source, &options).expect("build file should succeed");
        let rendered = fs::read_to_string(&artifact.path).expect("read generated LLVM IR");

        assert_eq!(artifact.path, output);
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
        assert!(rendered.contains("call i64 @ql_0_add_one(i64 41)"));
    }

    #[test]
    fn build_file_writes_object_with_mock_toolchain() {
        let dir = TestDir::new("ql-driver-object");
        let source = dir.write(
            "sample.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    return add_one(41)
}
"#,
        );
        let output = dir.path().join(format!(
            "artifacts/sample.{}",
            if cfg!(windows) { "obj" } else { "o" }
        ));
        let clang = mock_success_invocation(&dir);
        let options = BuildOptions {
            emit: BuildEmit::Object,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(clang),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect("object build should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated object placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-object");
        let leftovers = fs::read_dir(output.parent().expect("object output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen.ll"))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful object emission should clean up intermediate LLVM IR"
        );
    }

    #[test]
    fn build_file_writes_executable_with_mock_toolchain() {
        let dir = TestDir::new("ql-driver-exe");
        let source = dir.write(
            "sample.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    return add_one(41)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/sample.exe"
        } else {
            "artifacts/sample"
        });
        let clang = mock_success_invocation(&dir);
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(clang),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect("executable build should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful executable emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_async_main_executable_with_mock_toolchain() {
        let dir = TestDir::new("ql-driver-async-exe");
        let source = dir.write(
            "async_main.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_main.exe"
        } else {
            "artifacts/async_main"
        });
        let clang = mock_success_invocation(&dir);
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(clang),
                ..ToolchainOptions::default()
            },
        };

        let artifact =
            build_file(&source, &options).expect("async executable build should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful async executable emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_executable_with_async_main_fixed_array_for_await() {
        let dir = TestDir::new("ql-driver-async-exe-for-await-array");
        let source = dir.write(
            "async_main_for_await.ql",
            r#"
async fn main() -> Int {
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }
    return total
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_main_for_await.exe"
        } else {
            "artifacts/async_main_for_await"
        });
        let clang = mock_success_invocation(&dir);
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(clang),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with fixed-array for-await should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful async executable with fixed-array for-await should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_executable_with_async_main_nested_task_handle_results() {
        let dir = TestDir::new("ql-driver-async-exe-nested-task-handle");
        let source = dir.write(
            "async_nested_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let next = await outer()
    return await next
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_nested_task_handle.exe"
        } else {
            "artifacts/async_nested_task_handle"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with nested task-handle results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_tuple_task_handle_payload_results() {
        let dir = TestDir::new("ql-driver-async-exe-tuple-task-handle-payload");
        let source = dir.write(
            "async_tuple_task_handle_payload.ql",
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn main() -> Int {
    let pair = await outer()
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_tuple_task_handle_payload.exe"
        } else {
            "artifacts/async_tuple_task_handle_payload"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with tuple task-handle payload should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_array_task_handle_payload_results() {
        let dir = TestDir::new("ql-driver-async-exe-array-task-handle-payload");
        let source = dir.write(
            "async_array_task_handle_payload.ql",
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn main() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    let second = await tasks[1]
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_array_task_handle_payload.exe"
        } else {
            "artifacts/async_array_task_handle_payload"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with array task-handle payload should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_nested_aggregate_task_handle_payload_results() {
        let dir = TestDir::new("ql-driver-async-exe-nested-aggregate-task-handle-payload");
        let source = dir.write(
            "async_nested_aggregate_task_handle_payload.ql",
            r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_nested_aggregate_task_handle_payload.exe"
        } else {
            "artifacts/async_nested_aggregate_task_handle_payload"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with nested aggregate task-handle payload should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_helper_task_handle_flows() {
        let dir = TestDir::new("ql-driver-async-exe-helper-task-handle-flows");
        let source = dir.write(
            "async_helper_task_handle_flows.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn other() -> Int {
    return 2
}

fn schedule() -> Task[Int] {
    return worker()
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let direct = await schedule()

    let bound = schedule()
    let bound_value = await bound

    let spawned = spawn schedule()
    let spawned_value = await spawned

    let task = other()
    let forwarded = forward(task)
    let forwarded_value = await forwarded

    let next = worker()
    let running = spawn forward(next)
    let running_value = await running

    return direct + bound_value + spawned_value + forwarded_value + running_value
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_helper_task_handle_flows.exe"
        } else {
            "artifacts/async_helper_task_handle_flows"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with helper task-handle flows should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_helper_task_handle_flows() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-helper-task-handle-flows");
        let source = dir.write(
            "async_zero_sized_helper_task_handle_flows.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn other() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let direct = await schedule()

    let bound = schedule()
    let bound_value = await bound

    let spawned = spawn schedule()
    let spawned_value = await spawned

    let task = other()
    let forwarded = forward(task)
    let forwarded_value = await forwarded

    let next = worker()
    let running = spawn forward(next)
    let running_value = await running

    return score(direct)
        + score(bound_value)
        + score(spawned_value)
        + score(forwarded_value)
        + score(running_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_helper_task_handle_flows.exe"
        } else {
            "artifacts/async_zero_sized_helper_task_handle_flows"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized helper task-handle flows should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_local_returned_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-async-exe-local-return-task-handle-helper");
        let source = dir.write(
            "async_local_return_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    let task = worker()
    return task
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_local_return_task_handle.exe"
        } else {
            "artifacts/async_local_return_task_handle"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with local-return task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_direct_task_handles() {
        let dir = TestDir::new("ql-driver-async-exe-direct-task-handle");
        let source = dir.write(
            "async_direct_task_handle.ql",
            r#"
async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first_task = worker(1)
    let second_task = worker(2)
    let first = await first_task
    let second = await second_task
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_direct_task_handle.exe"
        } else {
            "artifacts/async_direct_task_handle"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with direct task handles should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_spawned_bound_task_handles() {
        let dir = TestDir::new("ql-driver-async-exe-spawn-bound-task-handle");
        let source = dir.write(
            "async_spawn_bound_task_handle.ql",
            r#"
async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first_task = worker(1)
    let second_task = worker(2)
    let first_running = spawn first_task
    let second_running = spawn second_task
    let first = await first_running
    let second = await second_running
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_bound_task_handle.exe"
        } else {
            "artifacts/async_spawn_bound_task_handle"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with spawned bound task handles should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_local_returned_zero_sized_task_handle_helpers()
    {
        let dir = TestDir::new("ql-driver-async-exe-local-return-zero-sized-task-handle-helper");
        let source = dir.write(
            "async_local_return_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_local_return_zero_sized_task_handle.exe"
        } else {
            "artifacts/async_local_return_zero_sized_task_handle"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with local-return zero-sized task-handle helpers should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_aggregate_results() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-aggregate-results");
        let source = dir.write(
            "async_zero_sized_aggregate_results.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

fn score(values: [Int; 0], value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await empty_values()
    let second = await wrapped()
    return score(first, second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_aggregate_results.exe"
        } else {
            "artifacts/async_zero_sized_aggregate_results"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized aggregate results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_spawned_zero_sized_aggregate_results() {
        let dir = TestDir::new("ql-driver-async-exe-spawn-zero-sized-aggregate-result");
        let source = dir.write(
            "async_spawn_zero_sized_aggregate_result.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let task = spawn worker()
    let first = await task
    return score(first)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_zero_sized_aggregate_result.exe"
        } else {
            "artifacts/async_spawn_zero_sized_aggregate_result"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with spawned zero-sized aggregate results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_aggregate_results() {
        let dir = TestDir::new("ql-driver-async-exe-aggregate-results");
        let source = dir.write(
            "async_aggregate_results.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 1)
}

async fn array_worker() -> [Int; 3] {
    return [2, 3, 4]
}

async fn pair_worker() -> Pair {
    return Pair { left: 5, right: 6 }
}

fn score_tuple(pair: (Bool, Int)) -> Int {
    if pair[0] {
        return pair[1]
    }
    return 0
}

fn score_array(values: [Int; 3]) -> Int {
    return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
    return pair.left + pair.right
}

async fn main() -> Int {
    let tuple_value = await tuple_worker()
    let array_value = await array_worker()
    let pair_value = await pair_worker()
    return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_aggregate_results.exe"
        } else {
            "artifacts/async_aggregate_results"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with aggregate results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_spawned_aggregate_results() {
        let dir = TestDir::new("ql-driver-async-exe-spawned-aggregate-results");
        let source = dir.write(
            "async_spawn_aggregate_results.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 1)
}

async fn array_worker() -> [Int; 3] {
    return [2, 3, 4]
}

async fn pair_worker() -> Pair {
    return Pair { left: 5, right: 6 }
}

fn score_tuple(pair: (Bool, Int)) -> Int {
    if pair[0] {
        return pair[1]
    }
    return 0
}

fn score_array(values: [Int; 3]) -> Int {
    return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
    return pair.left + pair.right
}

async fn main() -> Int {
    let tuple_task = spawn tuple_worker()
    let array_task = spawn array_worker()
    let pair_task = spawn pair_worker()
    let tuple_value = await tuple_task
    let array_value = await array_task
    let pair_value = await pair_task
    return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_aggregate_results.exe"
        } else {
            "artifacts/async_spawn_aggregate_results"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with spawned aggregate results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_recursive_aggregate_results() {
        let dir = TestDir::new("ql-driver-async-exe-recursive-aggregate-results");
        let source = dir.write(
            "async_recursive_aggregate_results.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score(result: (Pair, [Int; 2])) -> Int {
    return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
    let value = await worker()
    return score(value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_recursive_aggregate_results.exe"
        } else {
            "artifacts/async_recursive_aggregate_results"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with recursive aggregate results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_spawned_recursive_aggregate_results() {
        let dir = TestDir::new("ql-driver-async-exe-spawned-recursive-aggregate-results");
        let source = dir.write(
            "async_spawned_recursive_aggregate_results.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score(result: (Pair, [Int; 2])) -> Int {
    return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
    let task = spawn worker()
    let value = await task
    return score(value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawned_recursive_aggregate_results.exe"
        } else {
            "artifacts/async_spawned_recursive_aggregate_results"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with spawned recursive aggregate results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_recursive_aggregate_params() {
        let dir = TestDir::new("ql-driver-async-exe-recursive-aggregate-params");
        let source = dir.write(
            "async_recursive_aggregate_params.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn main() -> Int {
    return await worker(Pair { left: 1, right: 2 }, [3, 4])
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_recursive_aggregate_params.exe"
        } else {
            "artifacts/async_recursive_aggregate_params"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with recursive aggregate params should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_spawned_recursive_aggregate_params() {
        let dir = TestDir::new("ql-driver-async-exe-spawned-recursive-aggregate-params");
        let source = dir.write(
            "async_spawned_recursive_aggregate_params.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn main() -> Int {
    let task = spawn worker(Pair { left: 1, right: 2 }, [3, 4])
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawned_recursive_aggregate_params.exe"
        } else {
            "artifacts/async_spawned_recursive_aggregate_params"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with spawned recursive aggregate params should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_zero_sized_aggregate_params() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-aggregate-params");
        let source = dir.write(
            "async_zero_sized_aggregate_params.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn main() -> Int {
    return await worker([], Wrap { values: [] }, [[]])
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_aggregate_params.exe"
        } else {
            "artifacts/async_zero_sized_aggregate_params"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized aggregate params should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_spawned_zero_sized_aggregate_params() {
        let dir = TestDir::new("ql-driver-async-exe-spawned-zero-sized-aggregate-params");
        let source = dir.write(
            "async_spawned_zero_sized_aggregate_params.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn main() -> Int {
    let task = spawn worker([], Wrap { values: [] }, [[]])
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawned_zero_sized_aggregate_params.exe"
        } else {
            "artifacts/async_spawned_zero_sized_aggregate_params"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with spawned zero-sized aggregate params should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_projected_task_handle_awaits() {
        let dir = TestDir::new("ql-driver-async-exe-projected-task-handle-awaits");
        let source = dir.write(
            "async_projected_task_handle_awaits.ql",
            r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let tuple = (worker(1), worker(2))
    let tuple_first = await tuple[0]
    let tuple_second = await tuple[1]

    let array = [worker(3), worker(4)]
    let array_first = await array[0]
    let array_second = await array[1]

    let pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_first = await pair.left
    let struct_second = await pair.right

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_awaits.exe"
        } else {
            "artifacts/async_projected_task_handle_awaits"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with projected task-handle awaits should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_nested_task_handle_results() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-nested-task-handle");
        let source = dir.write(
            "async_zero_sized_nested_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Task[Wrap] {
    return worker()
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let next = await outer()
    let value = await next
    return score(value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_nested_task_handle.exe"
        } else {
            "artifacts/async_zero_sized_nested_task_handle"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized nested task-handle results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_struct_task_handle_payload_results()
    {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-struct-task-handle-payload");
        let source = dir.write(
            "async_zero_sized_struct_task_handle_payload.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending.first
    let second = await pending.second
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_struct_task_handle_payload.exe"
        } else {
            "artifacts/async_zero_sized_struct_task_handle_payload"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized struct task-handle payload should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_projected_task_handle_spawns() {
        let dir = TestDir::new("ql-driver-async-exe-projected-task-handle-spawns");
        let source = dir.write(
            "async_projected_task_handle_spawns.ql",
            r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let tuple = (worker(1), worker(2))
    let tuple_running = spawn tuple[0]
    let tuple_value = await tuple_running

    let array = [worker(3), worker(4)]
    let array_running = spawn array[0]
    let array_value = await array_running

    let pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_running = spawn pair.left
    let struct_value = await struct_running

    return score(tuple_value) + score(array_value) + score(struct_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_spawns.exe"
        } else {
            "artifacts/async_projected_task_handle_spawns"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with projected task-handle spawns should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_projected_task_handle_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-projected-task-handle-reinit");
        let source = dir.write(
            "async_projected_task_handle_reinit.ql",
            r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tuple = (worker(1), worker(2))
    let tuple_first = await tuple[0]
    tuple[0] = worker(7)
    let tuple_second = await tuple[0]

    var array = [worker(3), worker(4)]
    let array_first = await array[0]
    array[0] = worker(8)
    let array_second = await array[0]

    var pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_first = await pair.left
    pair.left = worker(9)
    let struct_second = await pair.left

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_reinit.exe"
        } else {
            "artifacts/async_projected_task_handle_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with projected task-handle reinit should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_projected_task_handle_conditional_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-projected-task-handle-conditional-reinit");
        let source = dir.write(
            "async_projected_task_handle_conditional_reinit.ql",
            r#"
async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(1), worker(2)]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker(7)
    }
    let final_value = await tasks[0]
    return score(final_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_conditional_reinit.exe"
        } else {
            "artifacts/async_projected_task_handle_conditional_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with projected task-handle conditional reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_projected_dynamic_task_handle_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-projected-dynamic-task-handle-reinit");
        let source = dir.write(
            "async_projected_dynamic_task_handle_reinit.ql",
            r#"
struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker(first + 1)
    let second = await tasks[slot.value]
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_dynamic_task_handle_reinit.exe"
        } else {
            "artifacts/async_projected_dynamic_task_handle_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with projected dynamic task-handle reinit should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_projected_dynamic_task_handle_conditional_reinit()
     {
        let dir =
            TestDir::new("ql-driver-async-exe-projected-dynamic-task-handle-conditional-reinit");
        let source = dir.write(
            "async_projected_dynamic_task_handle_conditional_reinit.ql",
            r#"
struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    if flag {
        let first = await tasks[slot.value]
        tasks[slot.value] = worker(first + 1)
    }
    let final_value = await tasks[slot.value]
    return score(final_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_dynamic_task_handle_conditional_reinit.exe"
        } else {
            "artifacts/async_projected_dynamic_task_handle_conditional_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with projected dynamic task-handle conditional reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_projected_task_handle_awaits() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-projected-task-handle-awaits");
        let source = dir.write(
            "async_zero_sized_projected_task_handle_awaits.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let tuple = (worker(), worker())
    let tuple_first = await tuple[0]
    let tuple_second = await tuple[1]

    let array = [worker(), worker()]
    let array_first = await array[0]
    let array_second = await array[1]

    let pair = TaskPair { left: worker(), right: worker() }
    let struct_first = await pair.left
    let struct_second = await pair.right

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_projected_task_handle_awaits.exe"
        } else {
            "artifacts/async_zero_sized_projected_task_handle_awaits"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized projected task-handle awaits should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_projected_task_handle_spawns() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-projected-task-handle-spawns");
        let source = dir.write(
            "async_zero_sized_projected_task_handle_spawns.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let tuple = (worker(), worker())
    let tuple_running = spawn tuple[0]
    let tuple_value = await tuple_running

    let array = [worker(), worker()]
    let array_running = spawn array[0]
    let array_value = await array_running

    let pair = TaskPair { left: worker(), right: worker() }
    let struct_running = spawn pair.left
    let struct_value = await struct_running

    return score(tuple_value) + score(array_value) + score(struct_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_projected_task_handle_spawns.exe"
        } else {
            "artifacts/async_zero_sized_projected_task_handle_spawns"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized projected task-handle spawns should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_projected_task_handle_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-projected-task-handle-reinit");
        let source = dir.write(
            "async_zero_sized_projected_task_handle_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    var tuple = (worker(), worker())
    let tuple_first = await tuple[0]
    tuple[0] = worker()
    let tuple_second = await tuple[0]

    var array = [worker(), worker()]
    let array_first = await array[0]
    array[0] = worker()
    let array_second = await array[0]

    var pair = TaskPair { left: worker(), right: worker() }
    let struct_first = await pair.left
    pair.left = worker()
    let struct_second = await pair.left

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_projected_task_handle_reinit.exe"
        } else {
            "artifacts/async_zero_sized_projected_task_handle_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized projected task-handle reinit should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_projected_task_handle_conditional_reinit()
     {
        let dir =
            TestDir::new("ql-driver-async-exe-zero-sized-projected-task-handle-conditional-reinit");
        let source = dir.write(
            "async_zero_sized_projected_task_handle_conditional_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    let final_value = await tasks[0]
    return score(final_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_projected_task_handle_conditional_reinit.exe"
        } else {
            "artifacts/async_zero_sized_projected_task_handle_conditional_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with zero-sized projected task-handle conditional reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_branch_spawned_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-branch-spawned-reinit");
        let source = dir.write(
            "async_branch_spawned_reinit.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn fresh_worker() -> Int {
    return 2
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    } else {
        task = fresh_worker()
    }
    let final_value = await task
    return score(final_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_branch_spawned_reinit.exe"
        } else {
            "artifacts/async_branch_spawned_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with branch spawned reinit should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_branch_spawned_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-branch-spawned-reinit");
        let source = dir.write(
            "async_zero_sized_branch_spawned_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    } else {
        task = fresh_worker()
    }
    let final_value = await task
    return score(final_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_branch_spawned_reinit.exe"
        } else {
            "artifacts/async_zero_sized_branch_spawned_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with zero-sized branch spawned reinit should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_reverse_branch_spawned_reinit() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-reverse-branch-spawned-reinit");
        let source = dir.write(
            "async_zero_sized_reverse_branch_spawned_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    }
    let final_value = await task
    return score(final_value)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_reverse_branch_spawned_reinit.exe"
        } else {
            "artifacts/async_zero_sized_reverse_branch_spawned_reinit"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with zero-sized reverse-branch spawned reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_conditional_async_call_spawns() {
        let dir = TestDir::new("ql-driver-async-exe-conditional-async-call-spawns");
        let source = dir.write(
            "async_conditional_async_call_spawns.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn choose(flag: Bool) -> Int {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Int {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await choose(true)
    let second = await choose_reverse(false)
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_conditional_async_call_spawns.exe"
        } else {
            "artifacts/async_conditional_async_call_spawns"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with conditional async-call spawns should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_conditional_async_call_spawns() {
        let dir = TestDir::new("ql-driver-async-exe-zero-sized-conditional-async-call-spawns");
        let source = dir.write(
            "async_zero_sized_conditional_async_call_spawns.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await choose(true)
    let second = await choose_reverse(false)
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_conditional_async_call_spawns.exe"
        } else {
            "artifacts/async_zero_sized_conditional_async_call_spawns"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with zero-sized conditional async-call spawns should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_conditional_helper_task_handle_spawns() {
        let dir = TestDir::new("ql-driver-async-exe-conditional-helper-task-handle-spawns");
        let source = dir.write(
            "async_conditional_helper_task_handle_spawns.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn choose(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_reverse(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Int {
    return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Int {
    return await choose_reverse(flag, worker())
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await helper(true)
    let second = await helper_reverse(false)
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_conditional_helper_task_handle_spawns.exe"
        } else {
            "artifacts/async_conditional_helper_task_handle_spawns"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("async executable with conditional helper task-handle spawns should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_executable_with_async_main_zero_sized_conditional_helper_task_handle_spawns()
     {
        let dir =
            TestDir::new("ql-driver-async-exe-zero-sized-conditional-helper-task-handle-spawns");
        let source = dir.write(
            "async_zero_sized_conditional_helper_task_handle_spawns.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_reverse(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Wrap {
    return await choose_reverse(flag, worker())
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await helper(true)
    let second = await helper_reverse(false)
    return score(first) + score(second)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_conditional_helper_task_handle_spawns.exe"
        } else {
            "artifacts/async_zero_sized_conditional_helper_task_handle_spawns"
        });
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options).expect(
            "async executable with zero-sized conditional helper task-handle spawns should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated executable placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_file_writes_dynamic_library_with_extern_c_definition_exports() {
        let dir = TestDir::new("ql-driver-dylib-extern-export");
        let source = dir.write(
            "ffi_export.ql",
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libffi_export.dylib"
        } else {
            "artifacts/libffi_export.so"
        });
        let options = BuildOptions {
            emit: BuildEmit::DynamicLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_dynamic_library_invocation(&dir, &["q_add"])),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("dynamic library build with extern definition export should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated dynamic library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-dylib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful dynamic library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_dynamic_library_with_default_export_header_sidecar() {
        let dir = TestDir::new("ql-driver-dylib-header-sidecar");
        let source = dir.write(
            "ffi_export.ql",
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libffi_export.dylib"
        } else {
            "artifacts/libffi_export.so"
        });
        let options = BuildOptions {
            emit: BuildEmit::DynamicLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: Some(BuildCHeaderOptions::default()),
            toolchain: ToolchainOptions {
                clang: Some(mock_dynamic_library_invocation(&dir, &["q_add"])),
                ..ToolchainOptions::default()
            },
        };

        let artifact =
            build_file(&source, &options).expect("dynamic library build with header should work");
        let header = artifact
            .c_header
            .expect("dynamic library build should return a generated header");
        let rendered = fs::read_to_string(&header.path).expect("read generated sidecar header");

        assert_eq!(artifact.path, output);
        assert_eq!(header.path, dir.path().join("artifacts/ffi_export.h"));
        assert_eq!(header.surface, CHeaderSurface::Exports);
        assert_eq!(header.exported_functions, 1);
        assert_eq!(header.imported_functions, 0);
        assert!(rendered.contains("#ifndef QLANG_FFI_EXPORT_H"));
        assert!(rendered.contains("int64_t q_add(int64_t left, int64_t right);"));
    }

    #[test]
    fn build_file_writes_dynamic_library_with_async_export_header_sidecar() {
        let dir = TestDir::new("ql-driver-dylib-async-export-header");
        let source = dir.write(
            "ffi_export_async.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export_async.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libffi_export_async.dylib"
        } else {
            "artifacts/libffi_export_async.so"
        });
        let options = BuildOptions {
            emit: BuildEmit::DynamicLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: Some(BuildCHeaderOptions {
                output: None,
                surface: CHeaderSurface::Exports,
            }),
            toolchain: ToolchainOptions {
                clang: Some(mock_dynamic_library_invocation(&dir, &["q_add"])),
                ..ToolchainOptions::default()
            },
        };

        let artifact = build_file(&source, &options)
            .expect("dynamic library build with async helpers and export header should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated dynamic library placeholder");
        let header = artifact
            .c_header
            .expect("dynamic library build should return a generated export header");
        let header_rendered = fs::read_to_string(&header.path).expect("read generated header");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-dylib");
        assert_eq!(header.path, dir.path().join("artifacts/ffi_export_async.h"));
        assert_eq!(header.surface, CHeaderSurface::Exports);
        assert_eq!(header.exported_functions, 1);
        assert_eq!(header.imported_functions, 0);
        assert!(header_rendered.contains("#ifndef QLANG_FFI_EXPORT_ASYNC_H"));
        assert!(header_rendered.contains("int64_t q_add(int64_t left, int64_t right);"));
        assert!(!header_rendered.contains("worker"));
        assert!(!header_rendered.contains("helper"));
    }

    #[test]
    fn build_file_writes_static_library_without_requiring_main() {
        let dir = TestDir::new("ql-driver-staticlib");
        let source = dir.write(
            "math.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn add_two(value: Int) -> Int {
    return add_one(add_one(value))
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/math.lib"
        } else {
            "artifacts/libmath.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect("static library build should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_extern_c_calls() {
        let dir = TestDir::new("ql-driver-staticlib-extern");
        let source = dir.write(
            "ffi_math.ql",
            r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_math.lib"
        } else {
            "artifacts/libffi_math.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact =
            build_file(&source, &options).expect("static library build with extern should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful extern-backed static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_supported_async_library_bodies() {
        let dir = TestDir::new("ql-driver-staticlib-async");
        let source = dir.write(
            "async_math.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_math.lib"
        } else {
            "artifacts/libasync_math.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with supported async library bodies should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful async static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-task-handle");
        let source = dir.write(
            "async_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    return await schedule()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_task_handle.lib"
        } else {
            "artifacts/libasync_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_nested_task_handle_async_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-nested-task-handle");
        let source = dir.write(
            "async_nested_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let next = await outer()
    return await next
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_nested_task_handle.lib"
        } else {
            "artifacts/libasync_nested_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with nested task-handle async results should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_tuple_task_handle_aggregate_async_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-tuple-task-handle-aggregate");
        let source = dir.write(
            "async_tuple_task_handle_aggregate.ql",
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn helper() -> Int {
    let pair = await outer()
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_tuple_task_handle_aggregate.lib"
        } else {
            "artifacts/libasync_tuple_task_handle_aggregate.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with tuple task-handle aggregate async results should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_array_task_handle_aggregate_async_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-array-task-handle-aggregate");
        let source = dir.write(
            "async_array_task_handle_aggregate.ql",
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn helper() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    let second = await tasks[1]
    return first + second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_array_task_handle_aggregate.lib"
        } else {
            "artifacts/libasync_array_task_handle_aggregate.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with array task-handle aggregate async results should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_nested_aggregate_task_handle_async_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-nested-aggregate-task-handle");
        let source = dir.write(
            "async_nested_aggregate_task_handle.ql",
            r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn helper() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_nested_aggregate_task_handle.lib"
        } else {
            "artifacts/libasync_nested_aggregate_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with nested aggregate task-handle async results should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_bound_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-bound-task-handle-helper");
        let source = dir.write(
            "async_bound_task_handle_helper.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = schedule()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_bound_task_handle_helper.lib"
        } else {
            "artifacts/libasync_bound_task_handle_helper.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with bound task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_local_returned_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-local-return-task-handle");
        let source = dir.write(
            "async_local_return_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    let task = worker()
    return task
}

async fn helper() -> Int {
    return await schedule()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_local_return_task_handle.lib"
        } else {
            "artifacts/libasync_local_return_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with local-return task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_zero_sized_recursive_aggregate_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-zero-sized-task-handle");
        let source = dir.write(
            "async_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    return await schedule()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_zero_sized_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with zero-sized task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_nested_zero_sized_task_handle_async_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-nested-zero-sized-task-handle");
        let source = dir.write(
            "async_nested_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let next = await outer()
    return await next
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_nested_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_nested_zero_sized_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with nested zero-sized task-handle async results should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_struct_task_handle_aggregate_async_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-struct-task-handle-aggregate");
        let source = dir.write(
            "async_struct_task_handle_aggregate.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

async fn helper() -> Wrap {
    let pending = await outer()
    await pending.first
    return await pending.second
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_struct_task_handle_aggregate.lib"
        } else {
            "artifacts/libasync_struct_task_handle_aggregate.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with struct task-handle aggregate async results should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_bound_zero_sized_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-bound-zero-sized-task-handle-helper");
        let source = dir.write(
            "async_bound_zero_sized_task_handle_helper.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let task = schedule()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_bound_zero_sized_task_handle_helper.lib"
        } else {
            "artifacts/libasync_bound_zero_sized_task_handle_helper.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with bound zero-sized task-handle helpers should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_local_returned_zero_sized_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-local-return-zero-sized-task-handle");
        let source = dir.write(
            "async_local_return_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

async fn helper() -> Wrap {
    return await schedule()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_local_return_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_local_return_zero_sized_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with local-return zero-sized task-handle helpers should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_forwarded_task_handle_arguments() {
        let dir = TestDir::new("ql-driver-staticlib-async-forward-task-handle");
        let source = dir.write(
            "async_forward_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn helper() -> Int {
    let task = worker()
    let forwarded = forward(task)
    return await forwarded
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_forward_task_handle.lib"
        } else {
            "artifacts/libasync_forward_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with forwarded task-handle arguments should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_forwarded_zero_sized_recursive_aggregate_task_handles()
    {
        let dir = TestDir::new("ql-driver-staticlib-async-forward-zero-sized-task-handle");
        let source = dir.write(
            "async_forward_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn helper() -> Wrap {
    let task = worker()
    let forwarded = forward(task)
    return await forwarded
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_forward_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_forward_zero_sized_task_handle.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with forwarded zero-sized task-handle arguments should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_supported_async_tuple_library_bodies() {
        let dir = TestDir::new("ql-driver-staticlib-async-tuple");
        let source = dir.write(
            "async_pair.ql",
            r#"
async fn worker() -> (Bool, Int) {
    return (true, 1)
}

async fn helper() -> (Bool, Int) {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_pair.lib"
        } else {
            "artifacts/libasync_pair.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options).expect(
            "static library build with supported async tuple library bodies should succeed",
        );
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful async tuple static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_supported_spawn_statements() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn");
        let source = dir.write(
            "async_spawn.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    spawn worker()
    return 0
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn.lib"
        } else {
            "artifacts/libasync_spawn.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with supported spawn statements should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful async spawn static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_async_export_header_sidecar() {
        let dir = TestDir::new("ql-driver-staticlib-async-export-header");
        let source = dir.write(
            "ffi_export_async.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export_async.lib"
        } else {
            "artifacts/libffi_export_async.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: Some(BuildCHeaderOptions {
                output: None,
                surface: CHeaderSurface::Exports,
            }),
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with async helpers and export header should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");
        let header = artifact
            .c_header
            .expect("static library build should return a generated export header");
        let header_rendered = fs::read_to_string(&header.path).expect("read generated header");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        assert_eq!(header.path, dir.path().join("artifacts/ffi_export_async.h"));
        assert_eq!(header.surface, CHeaderSurface::Exports);
        assert_eq!(header.exported_functions, 1);
        assert_eq!(header.imported_functions, 0);
        assert!(header_rendered.contains("#ifndef QLANG_FFI_EXPORT_ASYNC_H"));
        assert!(header_rendered.contains("int64_t q_add(int64_t left, int64_t right);"));
        assert!(!header_rendered.contains("worker"));
        assert!(!header_rendered.contains("helper"));
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful async export static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_import_header_sidecar() {
        let dir = TestDir::new("ql-driver-staticlib-import-header");
        let source = dir.write(
            "ffi_math.ql",
            r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_math.lib"
        } else {
            "artifacts/libffi_math.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: Some(BuildCHeaderOptions {
                output: None,
                surface: CHeaderSurface::Imports,
            }),
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact =
            build_file(&source, &options).expect("static library build with import header");
        let header = artifact
            .c_header
            .expect("static library build should return a generated header");
        let rendered = fs::read_to_string(&header.path).expect("read generated import header");

        assert_eq!(artifact.path, output);
        assert_eq!(header.path, dir.path().join("artifacts/ffi_math.imports.h"));
        assert_eq!(header.surface, CHeaderSurface::Imports);
        assert_eq!(header.exported_functions, 0);
        assert_eq!(header.imported_functions, 1);
        assert!(rendered.contains("#ifndef QLANG_FFI_MATH_IMPORTS_H"));
        assert!(rendered.contains("int64_t q_add(int64_t left, int64_t right);"));
    }

    #[test]
    fn build_file_writes_static_library_with_top_level_extern_c_calls() {
        let dir = TestDir::new("ql-driver-staticlib-top-level-extern");
        let source = dir.write(
            "ffi_math_top_level.ql",
            r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_math_top_level.lib"
        } else {
            "artifacts/libffi_math_top_level.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with top-level extern should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful top-level extern-backed static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_writes_static_library_with_extern_c_definition_exports() {
        let dir = TestDir::new("ql-driver-staticlib-extern-export");
        let source = dir.write(
            "ffi_export.ql",
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export.lib"
        } else {
            "artifacts/libffi_export.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let artifact = build_file(&source, &options)
            .expect("static library build with extern definition export should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
        let leftovers = fs::read_dir(output.parent().expect("output should have a parent"))
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".codegen."))
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "successful extern definition export static library emission should clean up intermediate artifacts"
        );
    }

    #[test]
    fn build_file_preserves_intermediate_ir_on_toolchain_failure() {
        let dir = TestDir::new("ql-driver-toolchain-fail");
        let source = dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join(format!(
            "artifacts/fail.{}",
            if cfg!(windows) { "obj" } else { "o" }
        ));
        let clang = mock_failure_invocation(&dir);
        let options = BuildOptions {
            emit: BuildEmit::Object,
            profile: BuildProfile::Debug,
            output: Some(output),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(clang),
                ..ToolchainOptions::default()
            },
        };

        let error = build_file(&source, &options).expect_err("object build should fail");
        let intermediate = error
            .intermediate_ir()
            .expect("toolchain failures should keep intermediate IR")
            .to_path_buf();

        assert!(matches!(&error, BuildError::Toolchain { .. }));
        assert!(
            intermediate.exists(),
            "intermediate LLVM IR should be preserved"
        );
        let rendered = fs::read_to_string(intermediate).expect("read preserved LLVM IR");
        assert!(rendered.contains("define i64 @ql_0_main()"));
    }

    #[test]
    fn build_file_preserves_ir_and_object_on_link_failure() {
        let dir = TestDir::new("ql-driver-link-fail");
        let source = dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/fail.exe"
        } else {
            "artifacts/fail"
        });
        let clang = mock_link_failure_invocation(&dir);
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            output: Some(output),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(clang),
                ..ToolchainOptions::default()
            },
        };

        let error = build_file(&source, &options).expect_err("executable build should fail");
        let preserved = error
            .preserved_artifacts()
            .expect("link failure should preserve intermediates");

        assert!(matches!(&error, BuildError::Toolchain { .. }));
        assert_eq!(preserved.len(), 2);
        assert!(preserved.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".codegen.ll"))
        }));
        assert!(preserved.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".codegen."))
                && path.extension().and_then(|ext| ext.to_str())
                    == Some(if cfg!(windows) { "obj" } else { "o" })
        }));
    }

    #[test]
    fn build_file_preserves_ir_and_object_on_archive_failure() {
        let dir = TestDir::new("ql-driver-archive-fail");
        let source = dir.write(
            "math.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/math.lib"
        } else {
            "artifacts/libmath.a"
        });
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_archive_failure_invocation(&dir)),
            },
        };

        let error = build_file(&source, &options).expect_err("static library build should fail");
        let preserved = error
            .preserved_artifacts()
            .expect("archive failure should preserve intermediates");

        assert!(matches!(&error, BuildError::Toolchain { .. }));
        assert_eq!(preserved.len(), 2);
        assert!(preserved.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".codegen.ll"))
        }));
        assert!(preserved.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".codegen."))
                && path.extension().and_then(|ext| ext.to_str())
                    == Some(if cfg!(windows) { "obj" } else { "o" })
        }));
    }

    #[test]
    fn build_file_surfaces_codegen_diagnostics() {
        let dir = TestDir::new("ql-driver-unsupported");
        let source = dir.write(
            "unsupported.ql",
            r#"
fn main() -> Int {
    let capture = () => 1
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("unsupported codegen should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support closure values yet"
        }));
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_writes_static_library_with_dynamic_task_handle_array_index_assignment() {
        let dir = TestDir::new("ql-driver-task-array-dynamic-index-assignment");
        let source = dir.write(
            "task_array_dynamic_index_assignment.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    tasks[index] = worker()
    return await tasks[0]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/task_array_dynamic_index_assignment.lib"
        } else {
            "artifacts/libtask_array_dynamic_index_assignment.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with dynamic task-handle array assignment should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_dynamic_task_handle_array_index_spawn_and_sibling_task_use()
     {
        let dir = TestDir::new("ql-driver-task-array-dynamic-index-spawn-sibling");
        let source = dir.write(
            "task_array_dynamic_index_spawn_sibling.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
    fallback: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    let pending = Pending {
        tasks: [worker(), worker()],
        fallback: worker(),
    }
    let running = spawn pending.tasks[index]
    let first = await running
    return await pending.fallback
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/task_array_dynamic_index_spawn_sibling.lib"
        } else {
            "artifacts/libtask_array_dynamic_index_spawn_sibling.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with dynamic task-handle spawn and sibling task use should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_same_immutable_dynamic_task_handle_reinit() {
        let dir = TestDir::new("ql-driver-task-array-dynamic-index-same-reinit");
        let source = dir.write(
            "task_array_dynamic_index_same_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[index]
    tasks[index] = worker()
    return await tasks[index]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/task_array_dynamic_index_same_reinit.lib"
        } else {
            "artifacts/libtask_array_dynamic_index_same_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with same immutable dynamic task-handle reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_same_projected_immutable_dynamic_task_handle_reinit() {
        let dir = TestDir::new("ql-driver-task-array-dynamic-index-projected-reinit");
        let source = dir.write(
            "task_array_dynamic_index_projected_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker()
    return await tasks[slot.value]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/task_array_dynamic_index_projected_reinit.lib"
        } else {
            "artifacts/libtask_array_dynamic_index_projected_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with same projected immutable dynamic task-handle reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_same_projected_immutable_dynamic_task_handle_conditional_reinit()
     {
        let dir = TestDir::new("ql-driver-task-array-dynamic-index-projected-conditional-reinit");
        let source = dir.write(
            "task_array_dynamic_index_projected_conditional_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool, index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    if flag {
        let first = await tasks[slot.value]
        tasks[slot.value] = worker()
    }
    return await tasks[slot.value]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/task_array_dynamic_index_projected_conditional_reinit.lib"
        } else {
            "artifacts/libtask_array_dynamic_index_projected_conditional_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with conditional same projected immutable dynamic task-handle reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_surfaces_dynamic_task_array_index_assignment_after_consume_diagnostic_once() {
        let dir = TestDir::new("ql-driver-task-array-dynamic-index-after-consume");
        let source = dir.write(
            "task_array_dynamic_index_assignment_after_consume.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[0]
    tasks[index] = worker()
    return await tasks[0]
}
"#,
        );

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: None,
                c_header: None,
                toolchain: ToolchainOptions::default(),
            },
        )
        .expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("task-array dynamic index after-consume diagnostics should be returned");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "local `tasks` may have been moved on another control-flow path"
                })
                .count(),
            1
        );
    }

    #[test]
    fn build_file_surfaces_function_value_diagnostics_without_panicking() {
        let dir = TestDir::new("ql-driver-function-values");
        let source = dir.write(
            "function_values.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    let f = add_one
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("unsupported codegen should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "LLVM IR backend foundation does not support first-class function values yet"
        }));
    }

    #[test]
    fn build_file_surfaces_async_function_codegen_diagnostics() {
        let dir = TestDir::new("ql-driver-async-unsupported");
        let source = dir.write(
            "async_main.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    return await worker()
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async codegen rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `async fn` yet"
        }));
    }

    #[test]
    fn build_file_writes_static_library_with_direct_async_call_handles() {
        let dir = TestDir::new("ql-driver-staticlib-direct-async-handle");
        let source = dir.write(
            "direct_async_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = worker()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/direct_async_handle.lib"
        } else {
            "artifacts/libdirect_async_handle.a"
        });

        let artifact = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with direct async handles should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_dedupes_runtime_and_codegen_async_diagnostics() {
        let dir = TestDir::new("ql-driver-async-diagnostic-dedupe");
        let source = dir.write(
            "async_main.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    return await worker()
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async codegen rejection should return diagnostics");
        let async_count = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.message == "LLVM IR backend foundation does not support `async fn` yet"
            })
            .count();

        assert_eq!(
            async_count, 2,
            "expected one async rejection per async function body without driver/codegen duplicates, got {diagnostics:?}"
        );
    }

    #[test]
    fn build_file_surfaces_async_runtime_operator_diagnostics() {
        let dir = TestDir::new("ql-driver-async-runtime-operators");
        let source = dir.write(
            "async_runtime_ops.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = spawn worker()
    return await worker()
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async runtime operator rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `async fn` yet"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `spawn` yet"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `await` yet"
        }));
    }

    #[test]
    fn build_file_surfaces_async_iteration_runtime_diagnostics() {
        let dir = TestDir::new("ql-driver-async-iteration-runtime");
        let source = dir.write(
            "async_for_await.ql",
            r#"
async fn main() -> Int {
    for await value in [1, 2, 3] {
        break
    }
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async iteration rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `async fn` yet"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "LLVM IR backend foundation does not support `for await` lowering yet"
        }));
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.message
                != "LLVM IR backend foundation requires the `executor-spawn` runtime hook before lowering `async fn main`"
                && diagnostic.message
                    != "LLVM IR backend foundation requires the `task-await` runtime hook before lowering `async fn main`"
                && diagnostic.message
                    != "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering `async fn main`"
        }));
    }

    #[test]
    fn build_file_surfaces_async_for_await_library_diagnostics_without_backend_noise() {
        let dir = TestDir::new("ql-driver-async-for-await-library-runtime");
        let source = dir.write(
            "async_for_await_library.ql",
            r#"
async fn helper() -> Int {
    for await value in (1, 2, 3) {
        break
    }
    return 0
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_for_await_library.lib"
        } else {
            "artifacts/libasync_for_await_library.a"
        });

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output),
                c_header: None,
                toolchain: ToolchainOptions::default(),
            },
        )
        .expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async for-await library rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "LLVM IR backend foundation does not support `for await` lowering yet"
        }));
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `for await` lowering yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.message != "LLVM IR backend foundation does not support `for` lowering yet"
                && diagnostic.message
                    != "LLVM IR backend foundation does not support array values yet"
                && diagnostic.message
                    != "LLVM IR backend foundation does not support `async fn` yet"
        }));
    }

    #[test]
    fn build_file_writes_static_library_with_fixed_array_for_await_bodies() {
        let dir = TestDir::new("ql-driver-staticlib-async-for-await-array");
        let source = dir.write(
            "async_for_await_array.ql",
            r#"
async fn helper() -> Int {
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }
    return total
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_for_await_array.lib"
        } else {
            "artifacts/libasync_for_await_array.a"
        });

        let artifact = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with fixed-array for-await should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated static library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_projected_zero_sized_task_handle_tuple_await() {
        let dir = TestDir::new("ql-driver-async-projected-await-task-handle-tuple");
        let source = dir.write(
            "async_projected_task_handle_tuple_await.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = (worker(), worker())
    return await pair[0]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_tuple_await.lib"
        } else {
            "artifacts/libasync_projected_task_handle_tuple_await.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with projected task-handle await should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_projected_zero_sized_task_handle_tuple_spawn() {
        let dir = TestDir::new("ql-driver-async-projected-spawn-task-handle-tuple");
        let source = dir.write(
            "async_projected_task_handle_tuple_spawn.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = (worker(), worker())
    let running = spawn pair[0]
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_tuple_spawn.lib"
        } else {
            "artifacts/libasync_projected_task_handle_tuple_spawn.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with projected task-handle spawn should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_sibling_projected_zero_sized_task_handle_tuple_awaits()
    {
        let dir = TestDir::new("ql-driver-async-projected-sibling-await-task-handle-tuple");
        let source = dir.write(
            "async_projected_task_handle_tuple_sibling_awaits.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = (worker(), worker())
    let first = await pair[0]
    return await pair[1]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_tuple_sibling_awaits.lib"
        } else {
            "artifacts/libasync_projected_task_handle_tuple_sibling_awaits.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with sibling projected tuple awaits should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_projected_zero_sized_task_handle_array_await() {
        let dir = TestDir::new("ql-driver-async-projected-await-task-handle-array");
        let source = dir.write(
            "async_projected_task_handle_array_await.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = [worker(), worker()]
    return await pair[0]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_array_await.lib"
        } else {
            "artifacts/libasync_projected_task_handle_array_await.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with projected fixed-array task-handle await should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_projected_zero_sized_task_handle_array_spawn() {
        let dir = TestDir::new("ql-driver-async-projected-spawn-task-handle-array");
        let source = dir.write(
            "async_projected_task_handle_array_spawn.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = [worker(), worker()]
    let running = spawn pair[0]
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_array_spawn.lib"
        } else {
            "artifacts/libasync_projected_task_handle_array_spawn.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with projected fixed-array task-handle spawn should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_sibling_projected_zero_sized_task_handle_array_awaits()
    {
        let dir = TestDir::new("ql-driver-async-projected-sibling-await-task-handle-array");
        let source = dir.write(
            "async_projected_task_handle_array_sibling_awaits.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = [worker(), worker()]
    let first = await pair[0]
    return await pair[1]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_array_sibling_awaits.lib"
        } else {
            "artifacts/libasync_projected_task_handle_array_sibling_awaits.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with sibling projected fixed-array awaits should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_projected_zero_sized_task_handle_struct_field_await() {
        let dir = TestDir::new("ql-driver-async-projected-await-task-handle-struct-field");
        let source = dir.write(
            "async_projected_task_handle_struct_field_await.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    task: Task[Wrap],
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = TaskPair { task: worker(), value: 1 }
    return await pair.task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_struct_field_await.lib"
        } else {
            "artifacts/libasync_projected_task_handle_struct_field_await.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with projected struct-field task-handle await should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_sibling_projected_zero_sized_task_handle_struct_field_awaits()
     {
        let dir = TestDir::new("ql-driver-async-projected-sibling-await-task-handle-struct-field");
        let source = dir.write(
            "async_projected_task_handle_struct_field_sibling_awaits.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = TaskPair { left: worker(), right: worker() }
    let first = await pair.left
    return await pair.right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_struct_field_sibling_awaits.lib"
        } else {
            "artifacts/libasync_projected_task_handle_struct_field_sibling_awaits.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with sibling projected struct-field awaits should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_reinitialized_projected_zero_sized_task_handle_tuple()
    {
        let dir = TestDir::new("ql-driver-async-projected-reinit-task-handle-tuple");
        let source = dir.write(
            "async_projected_task_handle_tuple_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    var pair = (worker(), worker())
    let first = await pair[0]
    pair[0] = worker()
    return await pair[0]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_tuple_reinit.lib"
        } else {
            "artifacts/libasync_projected_task_handle_tuple_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with projected tuple task-handle reinit should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_reinitialized_projected_zero_sized_task_handle_struct_field()
     {
        let dir = TestDir::new("ql-driver-async-projected-reinit-task-handle-struct-field");
        let source = dir.write(
            "async_projected_task_handle_struct_field_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    var pair = TaskPair { left: worker(), right: worker() }
    let first = await pair.left
    pair.left = worker()
    return await pair.left
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_struct_field_reinit.lib"
        } else {
            "artifacts/libasync_projected_task_handle_struct_field_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with projected struct-field task-handle reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_reinitialized_projected_zero_sized_task_handle_fixed_array()
     {
        let dir = TestDir::new("ql-driver-async-projected-reinit-task-handle-fixed-array");
        let source = dir.write(
            "async_projected_task_handle_fixed_array_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[0]
    tasks[0] = worker()
    return await tasks[0]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_fixed_array_reinit.lib"
        } else {
            "artifacts/libasync_projected_task_handle_fixed_array_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with projected fixed-array task-handle reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_dynamic_non_task_array_index_assignment() {
        let dir = TestDir::new("ql-driver-dynamic-array-index-assignment");
        let source = dir.write(
            "dynamic_array_index_assignment.ql",
            r#"
fn write_at(index: Int) -> Int {
    var values = [1, 2, 3]
    values[index] = 9
    return values[index]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/dynamic_array_index_assignment.lib"
        } else {
            "artifacts/libdynamic_array_index_assignment.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with dynamic non-task array assignment should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_nested_dynamic_non_task_array_index_assignment() {
        let dir = TestDir::new("ql-driver-dynamic-nested-array-index-assignment");
        let source = dir.write(
            "dynamic_nested_array_index_assignment.ql",
            r#"
fn write_cell(row: Int, col: Int) -> Int {
    var matrix = [[1, 2, 3], [4, 5, 6]]
    matrix[row][col] = 9
    return matrix[row][col]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/dynamic_nested_array_index_assignment.lib"
        } else {
            "artifacts/libdynamic_nested_array_index_assignment.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with nested dynamic non-task array assignment should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_conditionally_reinitialized_projected_zero_sized_task_handle_fixed_array()
     {
        let dir =
            TestDir::new("ql-driver-async-projected-conditional-reinit-task-handle-fixed-array");
        let source = dir.write(
            "async_projected_task_handle_fixed_array_conditional_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool) -> Wrap {
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    return await tasks[0]
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_fixed_array_conditional_reinit.lib"
        } else {
            "artifacts/libasync_projected_task_handle_fixed_array_conditional_reinit.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with conditional projected fixed-array task-handle reinit should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_projected_zero_sized_task_handle_struct_field_spawn() {
        let dir = TestDir::new("ql-driver-async-projected-spawn-task-handle-struct-field");
        let source = dir.write(
            "async_projected_task_handle_struct_field_spawn.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    task: Task[Wrap],
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = TaskPair { task: worker(), value: 1 }
    let running = spawn pair.task
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_projected_task_handle_struct_field_spawn.lib"
        } else {
            "artifacts/libasync_projected_task_handle_struct_field_spawn.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with projected struct-field task-handle spawn should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_surfaces_cleanup_and_for_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-cleanup-for-unsupported");
        let source = dir.write(
            "cleanup_for.ql",
            r#"
extern "c" fn first()

fn main() -> Int {
    defer first()
    for value in 0 {
        break
    }
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup and for codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `for` lowering yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_cleanup_and_match_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-cleanup-match-unsupported");
        let source = dir.write(
            "cleanup_match.ql",
            r#"
extern "c" fn first()

fn main() -> Int {
    let flag = true
    defer first()
    return match flag {
        true => 1,
        false => 0,
    }
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup and match codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `match` lowering yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_match_and_question_mark_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-match-question-unsupported");
        let source = dir.write(
            "match_question.ql",
            r#"
fn helper() -> Int {
    let flag = true
    return match flag {
        true => 1,
        false => 0,
    }
}

fn main() -> Int {
    return helper()?
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("match and question mark codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation encountered an opaque expression that still needs MIR elaboration"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `match` lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation only supports single-name binding patterns"
                })
                .count(),
            2
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `?` lowering yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_cleanup_and_question_mark_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-cleanup-question-mark-unsupported");
        let source = dir.write(
            "cleanup_question_mark.ql",
            r#"
extern "c" fn first()

fn helper() -> Int {
    return 1
}

fn main() -> Int {
    defer first()
    return helper()?
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup and question-mark codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `?` lowering yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_cleanup_and_closure_value_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-cleanup-closure-value-unsupported");
        let source = dir.write(
            "cleanup_closure_value.ql",
            r#"
extern "c" fn first()

fn main() -> Int {
    defer first()
    let capture = () => 1
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup and closure value codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support closure values yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_writes_static_library_with_async_struct_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-struct");
        let source = dir.write(
            "async_struct_result_library.ql",
            r#"
struct Pair {
    left: Bool,
    right: Int,
}

async fn worker() -> Pair {
    return Pair { right: 42, left: true }
}

async fn helper() -> Pair {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_struct_result_library.lib"
        } else {
            "artifacts/libasync_struct_result_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with async struct results should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_async_array_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-array");
        let source = dir.write(
            "async_array_result_library.ql",
            r#"
async fn worker() -> [Int; 3] {
    return [1, 2, 3]
}

async fn helper() -> [Int; 3] {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_array_result_library.lib"
        } else {
            "artifacts/libasync_array_result_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with async array results should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_async_recursive_aggregate_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-recursive-aggregate");
        let source = dir.write(
            "async_recursive_aggregate_library.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

async fn helper() -> (Pair, [Int; 2]) {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_recursive_aggregate_library.lib"
        } else {
            "artifacts/libasync_recursive_aggregate_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with recursive aggregate async results should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_async_recursive_aggregate_params() {
        let dir = TestDir::new("ql-driver-staticlib-async-aggregate-params");
        let source = dir.write(
            "async_recursive_param_library.ql",
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn helper() -> Int {
    return await worker(Pair { left: 1, right: 2 }, [3, 4])
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_recursive_param_library.lib"
        } else {
            "artifacts/libasync_recursive_param_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with recursive aggregate async params should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_async_zero_sized_aggregate_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-zero-sized-results");
        let source = dir.write(
            "async_zero_sized_result_library.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

async fn helper_values() -> [Int; 0] {
    return await empty_values()
}

async fn helper_wrap() -> Wrap {
    return await wrapped()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_result_library.lib"
        } else {
            "artifacts/libasync_zero_sized_result_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with zero-sized async aggregate results should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_async_zero_sized_aggregate_params() {
        let dir = TestDir::new("ql-driver-staticlib-async-zero-sized-params");
        let source = dir.write(
            "async_zero_sized_param_library.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn helper() -> Int {
    return await worker([], Wrap { values: [] }, [[]])
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_zero_sized_param_library.lib"
        } else {
            "artifacts/libasync_zero_sized_param_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with zero-sized async aggregate params should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawn_handle_awaits() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-handle");
        let source = dir.write(
            "async_spawn_value_library.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = spawn worker()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_value_library.lib"
        } else {
            "artifacts/libasync_spawn_value_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned task handles should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_bound_task_handles() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-bound-task-handle");
        let source = dir.write(
            "async_spawn_bound_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = worker()
    let running = spawn task
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_bound_task_handle.lib"
        } else {
            "artifacts/libasync_spawn_bound_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned bound task handles should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_bound_zero_sized_task_handles() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-bound-zero-sized-task-handle");
        let source = dir.write(
            "async_spawn_bound_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let task = worker()
    let running = spawn task
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_bound_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_spawn_bound_zero_sized_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned bound zero-sized task handles should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-helper-handle");
        let source = dir.write(
            "async_spawn_helper_library.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = spawn schedule()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_helper_library.lib"
        } else {
            "artifacts/libasync_spawn_helper_library.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_zero_sized_recursive_aggregate_task_handles() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-helper-zero-sized-task-handle");
        let source = dir.write(
            "async_spawn_helper_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let task = spawn schedule()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_helper_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_spawn_helper_zero_sized_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned zero-sized task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_conditionally_spawned_zero_sized_task_handle_helpers()
    {
        let dir =
            TestDir::new("ql-driver-staticlib-async-conditional-spawn-zero-sized-task-handle");
        let source = dir.write(
            "async_conditional_spawn_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_conditional_spawn_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_conditional_spawn_zero_sized_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with conditionally spawned zero-sized task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_reverse_branch_conditionally_spawned_zero_sized_task_handle_helpers()
     {
        let dir = TestDir::new(
            "ql-driver-staticlib-async-reverse-conditional-spawn-zero-sized-task-handle",
        );
        let source = dir.write(
            "async_reverse_conditional_spawn_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_reverse_conditional_spawn_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_reverse_conditional_spawn_zero_sized_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with reverse-branch conditionally spawned zero-sized task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_conditionally_spawned_zero_sized_async_call_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-conditional-spawn-zero-sized-async-call");
        let source = dir.write(
            "async_conditional_spawn_zero_sized_async_call.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_conditional_spawn_zero_sized_async_call.lib"
        } else {
            "artifacts/libasync_conditional_spawn_zero_sized_async_call.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with conditionally spawned zero-sized async call helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_reverse_branch_conditionally_spawned_zero_sized_async_call_helpers()
     {
        let dir = TestDir::new(
            "ql-driver-staticlib-async-reverse-conditional-spawn-zero-sized-async-call",
        );
        let source = dir.write(
            "async_reverse_conditional_spawn_zero_sized_async_call.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_reverse_conditional_spawn_zero_sized_async_call.lib"
        } else {
            "artifacts/libasync_reverse_conditional_spawn_zero_sized_async_call.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with reverse-branch conditionally spawned zero-sized async call helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_surfaces_zero_sized_branch_join_helper_consume_reinit_diagnostic_once() {
        let dir = TestDir::new("ql-driver-async-branch-join-helper-consume-reinit-unsupported");
        let source = dir.write(
            "async_branch_join_helper_consume_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        forward(task)
    } else {
        task = fresh_worker()
    }
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_branch_join_helper_consume_reinit.lib"
        } else {
            "artifacts/libasync_branch_join_helper_consume_reinit.a"
        });

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("branch-join helper diagnostics should be returned");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "local `task` may have been moved on another control-flow path"
                })
                .count(),
            1
        );
    }

    #[test]
    fn build_file_surfaces_zero_sized_reverse_branch_join_helper_consume_reinit_diagnostic_once() {
        let dir =
            TestDir::new("ql-driver-async-reverse-branch-join-helper-consume-reinit-unsupported");
        let source = dir.write(
            "async_reverse_branch_join_helper_consume_reinit.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        forward(task)
    }
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_reverse_branch_join_helper_consume_reinit.lib"
        } else {
            "artifacts/libasync_reverse_branch_join_helper_consume_reinit.a"
        });

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("branch-join helper diagnostics should be returned");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "local `task` may have been moved on another control-flow path"
                })
                .count(),
            1
        );
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_bound_task_handle_helpers() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-bound-task-handle-helper");
        let source = dir.write(
            "async_spawn_bound_task_handle_helper.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_bound_task_handle_helper.lib"
        } else {
            "artifacts/libasync_spawn_bound_task_handle_helper.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned bound task-handle helpers should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_bound_zero_sized_task_handle_helpers() {
        let dir =
            TestDir::new("ql-driver-staticlib-async-spawn-bound-zero-sized-task-handle-helper");
        let source = dir.write(
            "async_spawn_bound_zero_sized_task_handle_helper.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_bound_zero_sized_task_handle_helper.lib"
        } else {
            "artifacts/libasync_spawn_bound_zero_sized_task_handle_helper.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with spawned bound zero-sized task-handle helpers should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_forwarded_task_handle_arguments() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-forward-task-handle");
        let source = dir.write(
            "async_spawn_forward_task_handle.ql",
            r#"
async fn worker() -> Int {
    return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn helper() -> Int {
    let task = worker()
    let running = spawn forward(task)
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_forward_task_handle.lib"
        } else {
            "artifacts/libasync_spawn_forward_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned forwarded task-handle arguments should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_forwarded_zero_sized_task_handles() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-forward-zero-sized-task-handle");
        let source = dir.write(
            "async_spawn_forward_zero_sized_task_handle.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn helper() -> Wrap {
    let task = worker()
    let running = spawn forward(task)
    return await running
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_forward_zero_sized_task_handle.lib"
        } else {
            "artifacts/libasync_spawn_forward_zero_sized_task_handle.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect(
            "static library build with spawned forwarded zero-sized task handles should succeed",
        );
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_writes_static_library_with_spawned_zero_sized_aggregate_results() {
        let dir = TestDir::new("ql-driver-staticlib-async-spawn-zero-sized-aggregate-result");
        let source = dir.write(
            "async_spawn_zero_sized_aggregate_result.ql",
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let task = spawn worker()
    return await task
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_spawn_zero_sized_aggregate_result.lib"
        } else {
            "artifacts/libasync_spawn_zero_sized_aggregate_result.a"
        });

        build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_success_invocation(&dir)),
                    archiver: Some(mock_success_archiver_invocation(&dir)),
                },
            },
        )
        .expect("static library build with spawned zero-sized aggregate results should succeed");
        let rendered =
            fs::read_to_string(&output).expect("read generated static library placeholder");

        assert_eq!(rendered, "mock-staticlib");
    }

    #[test]
    fn build_file_surfaces_async_and_generic_codegen_diagnostics() {
        let dir = TestDir::new("ql-driver-async-generic-unsupported");
        let source = dir.write(
            "async_generic_main.ql",
            r#"
async fn main[T]() -> Int {
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async/generic codegen rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "LLVM IR backend foundation does not support generic functions yet"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `async fn` yet"
        }));
    }

    #[test]
    fn build_file_surfaces_async_and_unsafe_codegen_diagnostics() {
        let dir = TestDir::new("ql-driver-async-unsafe-unsupported");
        let source = dir.write(
            "async_unsafe_main.ql",
            r#"
async unsafe fn main() -> Int {
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("async/unsafe codegen rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `async fn` yet"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "LLVM IR backend foundation does not support `unsafe fn` bodies yet"
        }));
    }

    #[test]
    fn build_file_writes_dynamic_library_with_supported_async_library_bodies() {
        let dir = TestDir::new("ql-driver-async-dylib-supported");
        let source = dir.write(
            "async_dylib.ql",
            r#"
extern "c" pub fn q_export() -> Int {
    return 1
}

async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_dylib.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libasync_dylib.dylib"
        } else {
            "artifacts/libasync_dylib.so"
        });

        let artifact = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::DynamicLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_dynamic_library_invocation(&dir, &["q_export"])),
                    ..ToolchainOptions::default()
                },
            },
        )
        .expect("dynamic library build with supported async library bodies should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated dynamic library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-dylib");
    }

    #[test]
    fn build_file_writes_dynamic_library_with_fixed_array_for_await_bodies() {
        let dir = TestDir::new("ql-driver-async-for-await-dylib-array");
        let source = dir.write(
            "async_for_await_dylib.ql",
            r#"
extern "c" pub fn q_export() -> Int {
    return 1
}

async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    for await value in [1, 2, 3] {
        break
    }
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_for_await_dylib.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libasync_for_await_dylib.dylib"
        } else {
            "artifacts/libasync_for_await_dylib.so"
        });

        let artifact = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::DynamicLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: None,
                toolchain: ToolchainOptions {
                    clang: Some(mock_dynamic_library_invocation(&dir, &["q_export"])),
                    ..ToolchainOptions::default()
                },
            },
        )
        .expect("dynamic library build with fixed-array for-await should succeed");
        let rendered =
            fs::read_to_string(&artifact.path).expect("read generated dynamic library placeholder");

        assert_eq!(artifact.path, output);
        assert_eq!(rendered, "mock-dylib");
    }

    #[test]
    fn build_file_surfaces_async_for_await_diagnostics_for_dylib_non_array_iterables() {
        let dir = TestDir::new("ql-driver-async-for-await-dylib-non-array");
        let source = dir.write(
            "async_for_await_dylib_non_array.ql",
            r#"
extern "c" pub fn q_export() -> Int {
    return 1
}

async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    for await value in (1, 2, 3) {
        break
    }
    return await worker()
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/async_for_await_dylib_non_array.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libasync_for_await_dylib_non_array.dylib"
        } else {
            "artifacts/libasync_for_await_dylib_non_array.so"
        });

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::DynamicLibrary,
                profile: BuildProfile::Debug,
                output: Some(output),
                c_header: None,
                toolchain: ToolchainOptions::default(),
            },
        )
        .expect_err("dynamic library build with non-array for-await should still fail");
        let diagnostics = error
            .diagnostics()
            .expect("async for-await codegen rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "LLVM IR backend foundation does not support `for await` lowering yet"
        }));
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("does not support `async fn` yet")
                && !diagnostic.message.contains("does not support `await` yet")
                && !diagnostic.message.contains(
                    "requires at least one public top-level `extern \"c\"` function definition",
                )
        }));
    }

    #[test]
    fn build_file_surfaces_match_lowering_diagnostics() {
        let dir = TestDir::new("ql-driver-match-unsupported");
        let source = dir.write(
            "match_main.ql",
            r#"
fn main() -> Int {
    let flag = true
    return match flag {
        true => 1,
        false => 0,
    }
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("match codegen rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `match` lowering yet"
        }));
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_for_lowering_diagnostics() {
        let dir = TestDir::new("ql-driver-for-unsupported");
        let source = dir.write(
            "for_main.ql",
            r#"
fn main() -> Int {
    for value in 0 {
        break
    }
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("for codegen rejection should return diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "LLVM IR backend foundation does not support `for` lowering yet"
        }));
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_cleanup_lowering_diagnostics_once() {
        let dir = TestDir::new("ql-driver-cleanup-unsupported");
        let source = dir.write(
            "cleanup_main.ql",
            r#"
extern "c" fn first()

fn main() -> Int {
    defer first()
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
    }

    #[test]
    fn build_file_surfaces_cleanup_and_async_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-cleanup-async-unsupported");
        let source = dir.write(
            "cleanup_async.ql",
            r#"
extern "c" fn first()

async fn worker() -> Int {
    return 1
}

fn main() -> Int {
    defer first()
    return 0
}
"#,
        );

        let error = build_file(&source, &BuildOptions::default()).expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup and async codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `async fn` yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_surfaces_cleanup_and_for_await_codegen_diagnostics_once_each() {
        let dir = TestDir::new("ql-driver-cleanup-for-await-unsupported");
        let source = dir.write(
            "cleanup_for_await.ql",
            r#"
extern "c" fn first()

async fn helper() -> Int {
    defer first()
    for await value in (1, 2, 3) {
        break
    }
    return 0
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/cleanup_for_await.lib"
        } else {
            "artifacts/libcleanup_for_await.a"
        });

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::StaticLibrary,
                profile: BuildProfile::Debug,
                output: Some(output),
                c_header: None,
                toolchain: ToolchainOptions::default(),
            },
        )
        .expect_err("build should fail");
        let diagnostics = error
            .diagnostics()
            .expect("cleanup and for-await codegen rejection should return diagnostics");

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.message
                        == "LLVM IR backend foundation does not support `for await` lowering yet"
                })
                .count(),
            1
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("could not resolve LLVM type for local")
                && !diagnostic
                    .message
                    .contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn build_file_rejects_dynamic_libraries_without_public_extern_c_exports() {
        let dir = TestDir::new("ql-driver-dylib-no-exports");
        let source = dir.write(
            "library.ql",
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}
"#,
        );

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::DynamicLibrary,
                profile: BuildProfile::Debug,
                output: None,
                c_header: None,
                toolchain: ToolchainOptions::default(),
            },
        )
        .expect_err("dynamic library build should require a public extern export");

        match error {
            BuildError::InvalidInput(message) => assert!(message.contains(
                "requires at least one public top-level `extern \"c\"` function definition"
            )),
            other => panic!("expected invalid input error, got {other:?}"),
        }
    }

    #[test]
    fn build_file_rejects_c_header_sidecars_for_non_library_emits() {
        let dir = TestDir::new("ql-driver-header-non-library");
        let source = dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::Executable,
                profile: BuildProfile::Debug,
                output: None,
                c_header: Some(BuildCHeaderOptions::default()),
                toolchain: ToolchainOptions::default(),
            },
        )
        .expect_err("header sidecars should be rejected for executables");

        match error {
            BuildError::InvalidInput(message) => assert!(
                message.contains("only supports `dylib` and `staticlib`"),
                "unexpected invalid input message: {message}"
            ),
            other => panic!("expected invalid input error, got {other:?}"),
        }
    }

    #[test]
    fn build_file_removes_library_artifact_when_header_generation_fails() {
        let dir = TestDir::new("ql-driver-header-build-fail");
        let source = dir.write(
            "unsupported.ql",
            r#"
extern "c" pub fn q_print(value: Int) -> Void {
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/unsupported.lib"
        } else {
            "artifacts/libunsupported.a"
        });
        let header = dir.path().join("artifacts/unsupported.h");
        let options = BuildOptions {
            emit: BuildEmit::StaticLibrary,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: Some(BuildCHeaderOptions {
                output: Some(header.clone()),
                surface: CHeaderSurface::Imports,
            }),
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        let error = build_file(&source, &options)
            .expect_err("missing import surface should fail the sidecar build");
        match error {
            BuildError::InvalidInput(message) => assert!(
                message
                    .contains("does not define any imported `extern \"c\"` function declarations"),
                "unexpected invalid input message: {message}"
            ),
            other => panic!("expected invalid input error, got {other:?}"),
        }
        assert!(
            !output.exists(),
            "library artifact should be removed when sidecar generation fails"
        );
        assert!(
            !header.exists(),
            "header artifact should not be left behind on failure"
        );
    }

    #[test]
    fn build_file_rejects_header_output_path_equal_to_primary_artifact() {
        let dir = TestDir::new("ql-driver-header-output-collision");
        let source = dir.write(
            "ffi_export.ql",
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/ffi_export.dll"
        } else if cfg!(target_os = "macos") {
            "artifacts/libffi_export.dylib"
        } else {
            "artifacts/libffi_export.so"
        });

        let error = build_file(
            &source,
            &BuildOptions {
                emit: BuildEmit::DynamicLibrary,
                profile: BuildProfile::Debug,
                output: Some(output.clone()),
                c_header: Some(BuildCHeaderOptions {
                    output: Some(output.clone()),
                    surface: CHeaderSurface::Exports,
                }),
                toolchain: ToolchainOptions {
                    clang: Some(mock_dynamic_library_invocation(&dir, &["q_add"])),
                    ..ToolchainOptions::default()
                },
            },
        )
        .expect_err("header output path collisions should be rejected");

        match error {
            BuildError::InvalidInput(message) => assert!(
                message.contains("must differ from the primary artifact output"),
                "unexpected invalid input message: {message}"
            ),
            other => panic!("expected invalid input error, got {other:?}"),
        }
    }

    fn mock_success_invocation(dir: &TestDir) -> ProgramInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-clang-success.ps1",
                r#"
$out = $null
$isCompile = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-c') {
        $isCompile = $true
    }
    if ($args[$i] -eq '-o') {
        $out = $args[$i + 1]
    }
}
if ($null -eq $out) {
    Write-Error "missing -o"
    exit 1
}
$isShared = $false
foreach ($arg in $args) {
    if ($arg -eq '-shared' -or $arg -eq '-dynamiclib') {
        $isShared = $true
    }
}
if ($isCompile) {
    Set-Content -Path $out -NoNewline -Value "mock-object"
} elseif ($isShared) {
    Set-Content -Path $out -NoNewline -Value "mock-dylib"
} else {
    Set-Content -Path $out -NoNewline -Value "mock-executable"
}
"#,
            );
            ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
            ])
        } else {
            let script = dir.write(
                "mock-clang-success.sh",
                r#"out=""
is_compile=0
is_shared=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-shared" ] || [ "$1" = "-dynamiclib" ]; then
    is_shared=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
elif [ "$is_shared" -eq 1 ]; then
  printf 'mock-dylib' > "$out"
else
  printf 'mock-executable' > "$out"
fi
"#,
            );
            ProgramInvocation::new("/bin/sh").with_args_prefix(vec![script.display().to_string()])
        }
    }

    fn mock_success_archiver_invocation(dir: &TestDir) -> ArchiverInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-archiver-success.ps1",
                r#"
$out = $null
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -like '/OUT:*') {
        $out = $args[$i].Substring(5)
    }
}
if ($null -eq $out) {
    Write-Error "missing /OUT"
    exit 1
}
Set-Content -Path $out -NoNewline -Value "mock-staticlib"
"#,
            );
            ArchiverInvocation {
                program: ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                    "-ExecutionPolicy".to_owned(),
                    "Bypass".to_owned(),
                    "-File".to_owned(),
                    script.display().to_string(),
                ]),
                flavor: ArchiverFlavor::Lib,
            }
        } else {
            let script = dir.write(
                "mock-archiver-success.sh",
                r#"out="$2"
printf 'mock-staticlib' > "$out"
"#,
            );
            ArchiverInvocation {
                program: ProgramInvocation::new("/bin/sh")
                    .with_args_prefix(vec![script.display().to_string()]),
                flavor: ArchiverFlavor::Ar,
            }
        }
    }

    fn mock_failure_invocation(dir: &TestDir) -> ProgramInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-clang-fail.ps1",
                "Write-Error \"mock clang failure\"\nexit 9\n",
            );
            ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
            ])
        } else {
            let script = dir.write(
                "mock-clang-fail.sh",
                "echo 'mock clang failure' 1>&2\nexit 9\n",
            );
            ProgramInvocation::new("/bin/sh").with_args_prefix(vec![script.display().to_string()])
        }
    }

    fn mock_archive_failure_invocation(dir: &TestDir) -> ArchiverInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-archiver-fail.ps1",
                "Write-Error \"mock archive failure\"\nexit 8\n",
            );
            ArchiverInvocation {
                program: ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                    "-ExecutionPolicy".to_owned(),
                    "Bypass".to_owned(),
                    "-File".to_owned(),
                    script.display().to_string(),
                ]),
                flavor: ArchiverFlavor::Lib,
            }
        } else {
            let script = dir.write(
                "mock-archiver-fail.sh",
                "echo 'mock archive failure' 1>&2\nexit 8\n",
            );
            ArchiverInvocation {
                program: ProgramInvocation::new("/bin/sh")
                    .with_args_prefix(vec![script.display().to_string()]),
                flavor: ArchiverFlavor::Ar,
            }
        }
    }

    fn mock_link_failure_invocation(dir: &TestDir) -> ProgramInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-clang-link-fail.ps1",
                r#"
$out = $null
$isCompile = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-c') {
        $isCompile = $true
    }
    if ($args[$i] -eq '-o') {
        $out = $args[$i + 1]
    }
}
if ($null -eq $out) {
    Write-Error "missing -o"
    exit 1
}
if ($isCompile) {
    Set-Content -Path $out -NoNewline -Value "mock-object"
    exit 0
}
Write-Error "mock link failure"
exit 7
"#,
            );
            ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
            ])
        } else {
            let script = dir.write(
                "mock-clang-link-fail.sh",
                r#"out=""
is_compile=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
  exit 0
fi
echo 'mock link failure' 1>&2
exit 7
"#,
            );
            ProgramInvocation::new("/bin/sh").with_args_prefix(vec![script.display().to_string()])
        }
    }

    fn mock_dynamic_library_invocation(
        dir: &TestDir,
        expected_exports: &[&str],
    ) -> ProgramInvocation {
        if cfg!(windows) {
            let expected_exports = expected_exports
                .iter()
                .map(|symbol| format!("'/EXPORT:{symbol}'"))
                .collect::<Vec<_>>()
                .join(", ");
            let script = dir.write(
                "mock-clang-dylib-success.ps1",
                &format!(
                    r#"
$expectedExports = @({expected_exports})
$out = $null
$isCompile = $false
$isShared = $false
$seenExports = @()
for ($i = 0; $i -lt $args.Count; $i++) {{
    if ($args[$i] -eq '-c') {{
        $isCompile = $true
    }}
    if ($args[$i] -eq '-shared' -or $args[$i] -eq '-dynamiclib') {{
        $isShared = $true
    }}
    if ($args[$i] -eq '-o') {{
        $out = $args[$i + 1]
    }}
    if ($args[$i] -like '/EXPORT:*') {{
        $seenExports += $args[$i]
    }}
}}
if ($null -eq $out) {{
    Write-Error "missing -o"
    exit 1
}}
if ($isCompile) {{
    Set-Content -Path $out -NoNewline -Value "mock-object"
    exit 0
}}
if (-not $isShared) {{
    Write-Error "expected shared library link"
    exit 1
}}
foreach ($expected in $expectedExports) {{
    if (-not ($seenExports -contains $expected)) {{
        Write-Error "missing $expected"
        exit 1
    }}
}}
Set-Content -Path $out -NoNewline -Value "mock-dylib"
"#
                ),
            );
            ProgramInvocation::new("powershell.exe").with_args_prefix(vec![
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
            ])
        } else {
            let script = dir.write(
                "mock-clang-dylib-success.sh",
                r#"out=""
is_compile=0
is_shared=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-shared" ] || [ "$1" = "-dynamiclib" ]; then
    is_shared=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$out" = "" ]; then
  echo "missing -o" 1>&2
  exit 1
fi
if [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
elif [ "$is_shared" -eq 1 ]; then
  printf 'mock-dylib' > "$out"
else
  printf 'mock-executable' > "$out"
fi
"#,
            );
            ProgramInvocation::new("/bin/sh").with_args_prefix(vec![script.display().to_string()])
        }
    }
}
