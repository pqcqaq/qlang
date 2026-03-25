use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::analyze_source;
use ql_codegen_llvm::{CodegenError, CodegenInput, CodegenMode, emit_module};
use ql_diagnostics::Diagnostic;

use crate::toolchain::{ToolchainError, ToolchainOptions, discover_toolchain};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildEmit {
    LlvmIr,
    Object,
    Executable,
    StaticLibrary,
}

impl BuildEmit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlvmIr => "llvm-ir",
            Self::Object => "object",
            Self::Executable => "executable",
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
    pub toolchain: ToolchainOptions,
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

    let module_name = default_module_name(path);
    let ir = emit_module(CodegenInput {
        module_name: &module_name,
        mode: codegen_mode(options.emit),
        hir: analysis.hir(),
        mir: analysis.mir(),
        resolution: analysis.resolution(),
        typeck: analysis.typeck(),
    })
    .map_err(|error: CodegenError| BuildError::Diagnostics {
        path: path.to_path_buf(),
        source: source.clone(),
        diagnostics: error.into_diagnostics(),
    })?;

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
        BuildEmit::StaticLibrary => {
            build_static_library_file(&output_path, &ir, &options.toolchain)?;
        }
    }

    Ok(BuildArtifact {
        emit: options.emit,
        profile: options.profile,
        path: output_path,
    })
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

fn default_output_name(stem: &str, emit: BuildEmit) -> String {
    match emit {
        BuildEmit::LlvmIr => format!("{stem}.ll"),
        BuildEmit::Object => format!("{stem}.{}", object_extension()),
        BuildEmit::Executable => executable_name(stem),
        BuildEmit::StaticLibrary => static_library_name(stem),
    }
}

fn codegen_mode(emit: BuildEmit) -> CodegenMode {
    match emit {
        BuildEmit::LlvmIr | BuildEmit::Object | BuildEmit::Executable => CodegenMode::Program,
        BuildEmit::StaticLibrary => CodegenMode::Library,
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
        BuildEmit, BuildError, BuildOptions, BuildProfile, build_file, default_output_path,
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
            static_library,
            PathBuf::from(if cfg!(windows) {
                "D:/workspace/demo/target/ql/release/app.lib"
            } else {
                "D:/workspace/demo/target/ql/release/libapp.a"
            })
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
if ($isCompile) {
    Set-Content -Path $out -NoNewline -Value "mock-object"
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
}
