use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::analyze_source;
use ql_codegen_llvm::{emit_module, CodegenInput, CodegenMode};
use ql_diagnostics::{Diagnostic, Label};
use ql_runtime::{collect_runtime_hook_signatures, RuntimeCapability};

use crate::ffi::{
    emit_c_header_from_analysis, exported_c_symbol_names, CHeaderArtifact, CHeaderError,
    CHeaderOptions, CHeaderSurface,
};
use crate::toolchain::{discover_toolchain, ToolchainError, ToolchainOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildEmit {
    LlvmIr,
    Assembly,
    Object,
    Executable,
    DynamicLibrary,
    StaticLibrary,
}

impl BuildEmit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlvmIr => "llvm-ir",
            Self::Assembly => "assembly",
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
    build_file_with_link_inputs(path, options, &[])
}

pub fn build_file_with_link_inputs(
    path: &Path,
    options: &BuildOptions,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, BuildError> {
    if !path.is_file() {
        return Err(BuildError::InvalidInput(format!(
            "`{}` is not a file",
            path.display()
        )));
    }

    let source = fs::read_to_string(path).map_err(|error| BuildError::Io {
        path: path.to_path_buf(),
        error,
    })?;

    build_source_with_link_inputs(path, &source, options, additional_link_inputs)
}

pub fn build_source_with_link_inputs(
    path: &Path,
    source: &str,
    options: &BuildOptions,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, BuildError> {
    if options.c_header.is_some() && !build_emit_supports_c_header(options.emit) {
        return Err(BuildError::InvalidInput(format!(
            "build-side C header generation only supports `dylib` and `staticlib`, found `{}`",
            options.emit.as_str()
        )));
    }
    let source = source.to_owned();

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
    if matches!(
        options.emit,
        BuildEmit::Executable | BuildEmit::LlvmIr | BuildEmit::Object
    ) && analysis
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
        inline_runtime_support: options.emit == BuildEmit::DynamicLibrary,
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
        BuildEmit::Assembly => {
            build_assembly_file(&output_path, &ir, &options.toolchain)?;
        }
        BuildEmit::Object => {
            build_object_file(&output_path, &ir, &options.toolchain)?;
        }
        BuildEmit::Executable => {
            build_executable_file(
                &output_path,
                &ir,
                additional_link_inputs,
                &options.toolchain,
            )?;
        }
        BuildEmit::DynamicLibrary => {
            build_dynamic_library_file(
                &output_path,
                &ir,
                &exported_symbols,
                additional_link_inputs,
                &options.toolchain,
            )?;
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
        // The current async subset is open for staticlib, dylib, llvm-ir, assembly, object,
        // and executable builds.
        RuntimeCapability::AsyncFunctionBodies
        | RuntimeCapability::TaskAwait
        | RuntimeCapability::TaskSpawn
            if matches!(
                emit,
                BuildEmit::StaticLibrary
                    | BuildEmit::DynamicLibrary
                    | BuildEmit::Executable
                    | BuildEmit::LlvmIr
                    | BuildEmit::Assembly
                    | BuildEmit::Object
            ) =>
        {
            None
        }
        RuntimeCapability::AsyncIteration
            if matches!(
                emit,
                BuildEmit::StaticLibrary
                    | BuildEmit::DynamicLibrary
                    | BuildEmit::Executable
                    | BuildEmit::LlvmIr
                    | BuildEmit::Assembly
                    | BuildEmit::Object
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

fn build_assembly_file(
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

    if let Err(error) = toolchain.compile_llvm_ir_to_assembly(&intermediate_ir, output_path) {
        let _ = fs::remove_file(output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    cleanup_artifacts(&[intermediate_ir]);
    Ok(())
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
    additional_link_inputs: &[PathBuf],
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

    if let Err(error) = toolchain.link_object_to_executable_with_inputs(
        &intermediate_object,
        output_path,
        additional_link_inputs,
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

    match fs::remove_file(output_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            let _ = fs::remove_file(&intermediate_object);
            return Err(BuildError::Io {
                path: output_path.to_path_buf(),
                error,
            });
        }
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
    additional_link_inputs: &[PathBuf],
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

    if let Err(error) = toolchain.link_object_to_dynamic_library_with_inputs(
        &intermediate_object,
        output_path,
        exported_symbols,
        additional_link_inputs,
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
    if cfg!(windows) {
        "obj"
    } else {
        "o"
    }
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
        BuildEmit::Assembly => format!("{stem}.s"),
        BuildEmit::Object => format!("{stem}.{}", object_extension()),
        BuildEmit::Executable => executable_name(stem),
        BuildEmit::DynamicLibrary => dynamic_library_name(stem),
        BuildEmit::StaticLibrary => static_library_name(stem),
    }
}

fn codegen_mode(emit: BuildEmit) -> CodegenMode {
    match emit {
        BuildEmit::LlvmIr | BuildEmit::Assembly | BuildEmit::Object | BuildEmit::Executable => {
            CodegenMode::Program
        }
        BuildEmit::DynamicLibrary | BuildEmit::StaticLibrary => CodegenMode::Library,
    }
}

#[cfg(test)]
#[path = "build/tests.rs"]
mod tests;
