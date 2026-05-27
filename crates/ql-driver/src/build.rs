use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ql_analysis::analyze_source;
use ql_codegen_llvm::{CodegenInput, CodegenMode, emit_module};
use ql_diagnostics::{Diagnostic, Label};
use ql_runtime::{RuntimeCapability, collect_runtime_hook_signatures};

use crate::ffi::{
    CHeaderArtifact, CHeaderError, CHeaderOptions, CHeaderSurface, emit_c_header_from_analysis,
    exported_c_symbol_names,
};
use crate::toolchain::{ToolchainError, ToolchainOptions, discover_toolchain};
use crate::{replace_file_atomically, write_file_atomically};

const BUILD_OUTPUT_LOCK_TIMEOUT: Duration = Duration::from_secs(120);
const BUILD_OUTPUT_LOCK_RETRY: Duration = Duration::from_millis(25);

#[derive(Debug)]
struct BuildCodegenPreparation {
    source: String,
    analysis: ql_analysis::Analysis,
    ir: String,
    exported_symbols: Vec<String>,
}

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

    let codegen = prepare_build_codegen(path, source, options.emit)?;

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

    let mut locked_paths = vec![output_path.clone()];
    locked_paths.extend(additional_link_inputs.iter().cloned());
    if let Some(header_options) = c_header_options.as_ref()
        && let Some(header_path) = header_options.output.as_ref()
    {
        locked_paths.push(header_path.clone());
    }
    let _output_locks = acquire_build_output_locks(locked_paths)?;

    match options.emit {
        BuildEmit::LlvmIr => {
            write_file_atomically(&output_path, codegen.ir.as_str()).map_err(|error| {
                BuildError::Io {
                    path: output_path.clone(),
                    error,
                }
            })?;
        }
        BuildEmit::Assembly => {
            build_assembly_file(&output_path, &codegen.ir, &options.toolchain)?;
        }
        BuildEmit::Object => {
            build_object_file(&output_path, &codegen.ir, &options.toolchain)?;
        }
        BuildEmit::Executable => {
            build_executable_file(
                &output_path,
                &codegen.ir,
                additional_link_inputs,
                &options.toolchain,
            )?;
        }
        BuildEmit::DynamicLibrary => {
            build_dynamic_library_file(
                &output_path,
                &codegen.ir,
                &codegen.exported_symbols,
                additional_link_inputs,
                &options.toolchain,
            )?;
        }
        BuildEmit::StaticLibrary => {
            build_static_library_file(&output_path, &codegen.ir, &options.toolchain)?;
        }
    }

    let c_header = match c_header_options {
        Some(ref header_options) => {
            let header_path = header_options
                .output
                .clone()
                .expect("build-side C header output path should be resolved");
            match emit_c_header_from_analysis(
                path,
                &codegen.source,
                &codegen.analysis,
                header_options,
            ) {
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

fn prepare_build_codegen(
    path: &Path,
    source: &str,
    emit: BuildEmit,
) -> Result<BuildCodegenPreparation, BuildError> {
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

    let exported_symbols = exported_symbols_for_emit(&analysis, emit)?;
    let runtime_diagnostics = runtime_requirement_diagnostics(&analysis, emit);
    let runtime_hooks = collect_runtime_hooks_for_codegen(&analysis, emit);
    let module_name = default_module_name(path);
    let ir = match emit_module(CodegenInput {
        module_name: &module_name,
        mode: codegen_mode(emit),
        inline_runtime_support: emit == BuildEmit::DynamicLibrary,
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
                    source,
                    diagnostics: runtime_diagnostics,
                });
            }
            ir
        }
        Err(error) => {
            return Err(BuildError::Diagnostics {
                path: path.to_path_buf(),
                source,
                diagnostics: merge_unique_diagnostics(
                    error.into_diagnostics(),
                    &runtime_diagnostics,
                ),
            });
        }
    };

    Ok(BuildCodegenPreparation {
        source,
        analysis,
        ir,
        exported_symbols,
    })
}

fn exported_symbols_for_emit(
    analysis: &ql_analysis::Analysis,
    emit: BuildEmit,
) -> Result<Vec<String>, BuildError> {
    if emit != BuildEmit::DynamicLibrary {
        return Ok(Vec::new());
    }

    let symbols = exported_c_symbol_names(analysis.hir());
    if symbols.is_empty() {
        return Err(BuildError::InvalidInput(
            "dynamic library emission currently requires at least one public top-level `extern \"c\"` function definition"
                .to_owned(),
        ));
    }
    Ok(symbols)
}

fn collect_runtime_hooks_for_codegen(
    analysis: &ql_analysis::Analysis,
    emit: BuildEmit,
) -> Vec<ql_runtime::RuntimeHookSignature> {
    let mut runtime_capabilities = analysis
        .runtime_requirements()
        .iter()
        .map(|requirement| requirement.capability)
        .collect::<Vec<_>>();
    if emit_requires_async_main_runtime_hooks(analysis, emit) {
        if !runtime_capabilities.contains(&RuntimeCapability::TaskSpawn) {
            runtime_capabilities.push(RuntimeCapability::TaskSpawn);
        }
        if !runtime_capabilities.contains(&RuntimeCapability::TaskAwait) {
            runtime_capabilities.push(RuntimeCapability::TaskAwait);
        }
    }
    collect_runtime_hook_signatures(runtime_capabilities)
}

fn emit_requires_async_main_runtime_hooks(
    analysis: &ql_analysis::Analysis,
    emit: BuildEmit,
) -> bool {
    matches!(
        emit,
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
}

pub struct BuildOutputLock {
    path: PathBuf,
}

impl BuildOutputLock {
    fn acquire(path: PathBuf) -> Result<Self, BuildError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| BuildError::Io {
                path: parent.to_path_buf(),
                error,
            })?;
        }

        let started = Instant::now();
        loop {
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if started.elapsed() >= BUILD_OUTPUT_LOCK_TIMEOUT {
                        return Err(BuildError::Io {
                            path,
                            error: io::Error::new(
                                io::ErrorKind::WouldBlock,
                                format!(
                                    "timed out waiting for ql build output lock after {}s",
                                    BUILD_OUTPUT_LOCK_TIMEOUT.as_secs()
                                ),
                            ),
                        });
                    }
                    thread::sleep(BUILD_OUTPUT_LOCK_RETRY);
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    if started.elapsed() >= BUILD_OUTPUT_LOCK_TIMEOUT {
                        return Err(BuildError::Io { path, error });
                    }
                    thread::sleep(BUILD_OUTPUT_LOCK_RETRY);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if started.elapsed() >= BUILD_OUTPUT_LOCK_TIMEOUT {
                        return Err(BuildError::Io { path, error });
                    }
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|error| BuildError::Io {
                            path: parent.to_path_buf(),
                            error,
                        })?;
                    }
                    thread::sleep(BUILD_OUTPUT_LOCK_RETRY);
                }
                Err(error) => {
                    return Err(BuildError::Io { path, error });
                }
            }
        }
    }
}

impl Drop for BuildOutputLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

pub fn acquire_build_output_locks(
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<BuildOutputLock>, BuildError> {
    let lock_paths = paths
        .into_iter()
        .map(|path| build_output_lock_path(&path))
        .collect::<BTreeSet<_>>();
    let mut locks = Vec::with_capacity(lock_paths.len());
    for path in lock_paths {
        locks.push(BuildOutputLock::acquire(path)?);
    }
    Ok(locks)
}

fn build_output_lock_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("artifact");
    parent.join(format!(
        ".{}.ql-build.lock",
        sanitize_lock_file_name(file_name)
    ))
}

fn sanitize_lock_file_name(file_name: &str) -> String {
    let sanitized = file_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "artifact".to_owned()
    } else {
        sanitized
    }
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
    let temp_output_path = final_artifact_temp_path(output_path);
    fs::write(&intermediate_ir, ir).map_err(|error| BuildError::Io {
        path: intermediate_ir.clone(),
        error,
    })?;

    let toolchain = discover_toolchain(toolchain_options)
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;

    if let Err(error) = toolchain.compile_llvm_ir_to_assembly(&intermediate_ir, &temp_output_path) {
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    promote_final_artifact(output_path, &temp_output_path, &[intermediate_ir.clone()])?;
    cleanup_artifacts(&[intermediate_ir]);
    Ok(())
}

fn build_object_file(
    output_path: &Path,
    ir: &str,
    toolchain_options: &ToolchainOptions,
) -> Result<(), BuildError> {
    let intermediate_ir = intermediate_ir_path(output_path);
    let temp_output_path = final_artifact_temp_path(output_path);
    fs::write(&intermediate_ir, ir).map_err(|error| BuildError::Io {
        path: intermediate_ir.clone(),
        error,
    })?;

    let toolchain = discover_toolchain(toolchain_options)
        .map_err(|error| toolchain_failure(error, vec![intermediate_ir.clone()]))?;

    if let Err(error) = toolchain.compile_llvm_ir_to_object(&intermediate_ir, &temp_output_path) {
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    promote_final_artifact(output_path, &temp_output_path, &[intermediate_ir.clone()])?;
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
    let temp_output_path = final_artifact_temp_path(output_path);
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
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    if let Err(error) = toolchain.link_object_to_executable_with_inputs(
        &intermediate_object,
        &temp_output_path,
        additional_link_inputs,
    ) {
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(
            error,
            vec![intermediate_ir, intermediate_object],
        ));
    }

    promote_final_artifact(
        output_path,
        &temp_output_path,
        &[intermediate_ir.clone(), intermediate_object.clone()],
    )?;
    cleanup_artifacts(&[intermediate_ir, intermediate_object]);
    Ok(())
}

fn build_static_library_file(
    output_path: &Path,
    ir: &str,
    toolchain_options: &ToolchainOptions,
) -> Result<(), BuildError> {
    let intermediate_ir = intermediate_ir_path(output_path);
    let temp_output_path = final_artifact_temp_path(output_path);
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
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    if let Err(error) =
        toolchain.archive_object_to_static_library(&intermediate_object, &temp_output_path)
    {
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(
            error,
            vec![intermediate_ir, intermediate_object],
        ));
    }

    promote_final_artifact(
        output_path,
        &temp_output_path,
        &[intermediate_ir.clone(), intermediate_object.clone()],
    )?;
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
    let temp_output_path = final_artifact_temp_path(output_path);
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
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(error, vec![intermediate_ir]));
    }

    if let Err(error) = toolchain.link_object_to_dynamic_library_with_inputs(
        &intermediate_object,
        &temp_output_path,
        exported_symbols,
        additional_link_inputs,
    ) {
        let _ = fs::remove_file(&temp_output_path);
        return Err(toolchain_failure(
            error,
            vec![intermediate_ir, intermediate_object],
        ));
    }

    promote_final_artifact(
        output_path,
        &temp_output_path,
        &[intermediate_ir.clone(), intermediate_object.clone()],
    )?;
    cleanup_artifacts(&[intermediate_ir, intermediate_object]);
    Ok(())
}

fn promote_final_artifact(
    output_path: &Path,
    temp_output_path: &Path,
    cleanup_on_error: &[PathBuf],
) -> Result<(), BuildError> {
    replace_file_atomically(output_path, temp_output_path).map_err(|error| {
        cleanup_artifacts(cleanup_on_error);
        BuildError::Io {
            path: output_path.to_path_buf(),
            error,
        }
    })
}

fn intermediate_ir_path(output_path: &Path) -> PathBuf {
    intermediate_artifact_path(output_path, "ll")
}

fn intermediate_object_path(output_path: &Path) -> PathBuf {
    intermediate_artifact_path(output_path, object_extension())
}

fn final_artifact_temp_path(output_path: &Path) -> PathBuf {
    let extension = output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("artifact");
    intermediate_artifact_path(output_path, extension)
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
mod build_output_lock_tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{acquire_build_output_locks, build_output_lock_path};

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
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn output_lock_uses_artifact_scoped_lock_directory() {
        let dir = TestDir::new("ql-driver-output-lock-path");
        let artifact = dir.path().join("target/ql/debug/lib.core.lib");
        let lock_path = build_output_lock_path(&artifact);

        let locks = acquire_build_output_locks(vec![artifact]).expect("acquire build output lock");

        assert!(lock_path.is_dir(), "lock directory should be created");
        drop(locks);
        assert!(
            !lock_path.exists(),
            "lock directory should be removed when the guard drops"
        );
    }

    #[test]
    fn output_lock_serializes_same_artifact_path() {
        let dir = TestDir::new("ql-driver-output-lock-serial");
        let artifact = dir.path().join("target/ql/debug/lib.lib");
        let lock_path = build_output_lock_path(&artifact);
        let first = acquire_build_output_locks(vec![artifact.clone()])
            .expect("acquire first build output lock");

        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            started_tx.send(()).expect("send worker started");
            let _second =
                acquire_build_output_locks(vec![artifact]).expect("acquire second output lock");
            done_tx.send(()).expect("send worker done");
        });

        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("worker should start");
        assert!(
            done_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "second lock acquisition should wait while first guard is alive"
        );

        drop(first);
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("second lock should acquire after first guard drops");
        worker.join().expect("worker should finish");
        assert!(
            !lock_path.exists(),
            "lock directory should be removed after both guards drop"
        );
    }
}

#[cfg(test)]
#[path = "build/tests.rs"]
mod tests;
