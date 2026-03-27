use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use ql_analysis::{analyze_source as analyze_semantics, parse_errors_to_diagnostics};
use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_driver::{
    BuildCHeaderOptions, BuildEmit, BuildError, BuildOptions, BuildProfile, CHeaderError,
    CHeaderOptions, CHeaderSurface, build_file, emit_c_header,
};
use ql_fmt::format_source;
use ql_runtime::collect_runtime_hooks;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run() -> Result<(), u8> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Err(1);
    };

    match command.as_str() {
        "check" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql check` expects a file or directory path");
                return Err(1);
            };
            check_path(Path::new(&path))
        }
        "fmt" => {
            let mut write = false;
            let mut path = None;
            for arg in args {
                if arg == "--write" {
                    write = true;
                } else {
                    path = Some(arg);
                }
            }

            let Some(path) = path else {
                eprintln!("error: `ql fmt` expects a file path");
                return Err(1);
            };

            format_path(Path::new(&path), write)
        }
        "mir" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql mir` expects a file path");
                return Err(1);
            };

            render_mir_path(Path::new(&path))
        }
        "ownership" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql ownership` expects a file path");
                return Err(1);
            };

            render_ownership_path(Path::new(&path))
        }
        "runtime" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql runtime` expects a file path");
                return Err(1);
            };

            render_runtime_requirements_path(Path::new(&path))
        }
        "build" => {
            let Some(path) = args.next() else {
                eprintln!("error: `ql build` expects a file path");
                return Err(1);
            };

            let mut options = BuildOptions::default();
            let remaining = args.collect::<Vec<_>>();
            let mut index = 0;

            while index < remaining.len() {
                match remaining[index].as_str() {
                    "--emit" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --emit` expects a value");
                            return Err(1);
                        };
                        match value.as_str() {
                            "llvm-ir" => options.emit = BuildEmit::LlvmIr,
                            "obj" => options.emit = BuildEmit::Object,
                            "exe" => options.emit = BuildEmit::Executable,
                            "dylib" => options.emit = BuildEmit::DynamicLibrary,
                            "staticlib" => options.emit = BuildEmit::StaticLibrary,
                            other => {
                                eprintln!("error: unsupported build emit target `{other}`");
                                return Err(1);
                            }
                        }
                    }
                    "--release" => {
                        options.profile = BuildProfile::Release;
                    }
                    "-o" | "--output" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --output` expects a file path");
                            return Err(1);
                        };
                        options.output = Some(PathBuf::from(value));
                    }
                    "--header" => {
                        options
                            .c_header
                            .get_or_insert_with(BuildCHeaderOptions::default);
                    }
                    "--header-surface" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!(
                                "error: `ql build --header-surface` expects `exports`, `imports`, or `both`"
                            );
                            return Err(1);
                        };
                        let Some(surface) = CHeaderSurface::parse(value) else {
                            eprintln!("error: unsupported `ql build` header surface `{value}`");
                            return Err(1);
                        };
                        let header = options
                            .c_header
                            .get_or_insert_with(BuildCHeaderOptions::default);
                        header.surface = surface;
                    }
                    "--header-output" => {
                        index += 1;
                        let Some(value) = remaining.get(index) else {
                            eprintln!("error: `ql build --header-output` expects a file path");
                            return Err(1);
                        };
                        let header = options
                            .c_header
                            .get_or_insert_with(BuildCHeaderOptions::default);
                        header.output = Some(PathBuf::from(value));
                    }
                    other => {
                        eprintln!("error: unknown `ql build` option `{other}`");
                        return Err(1);
                    }
                }

                index += 1;
            }

            build_path(Path::new(&path), &options)
        }
        "ffi" => {
            let Some(subcommand) = args.next() else {
                eprintln!("error: `ql ffi` expects a subcommand");
                return Err(1);
            };

            match subcommand.as_str() {
                "header" => {
                    let Some(path) = args.next() else {
                        eprintln!("error: `ql ffi header` expects a file path");
                        return Err(1);
                    };

                    let mut options = CHeaderOptions::default();
                    let remaining = args.collect::<Vec<_>>();
                    let mut index = 0;

                    while index < remaining.len() {
                        match remaining[index].as_str() {
                            "-o" | "--output" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql ffi header --output` expects a file path"
                                    );
                                    return Err(1);
                                };
                                options.output = Some(PathBuf::from(value));
                            }
                            "--surface" => {
                                index += 1;
                                let Some(value) = remaining.get(index) else {
                                    eprintln!(
                                        "error: `ql ffi header --surface` expects `exports`, `imports`, or `both`"
                                    );
                                    return Err(1);
                                };
                                let Some(surface) = CHeaderSurface::parse(value) else {
                                    eprintln!(
                                        "error: unsupported `ql ffi header` surface `{value}`"
                                    );
                                    return Err(1);
                                };
                                options.surface = surface;
                            }
                            other => {
                                eprintln!("error: unknown `ql ffi header` option `{other}`");
                                return Err(1);
                            }
                        }

                        index += 1;
                    }

                    emit_c_header_path(Path::new(&path), &options)
                }
                other => {
                    eprintln!("error: unknown `ql ffi` subcommand `{other}`");
                    print_usage();
                    Err(1)
                }
            }
        }
        _ => {
            eprintln!("error: unknown command `{command}`");
            print_usage();
            Err(1)
        }
    }
}

fn check_path(path: &Path) -> Result<(), u8> {
    let files = collect_ql_files(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;

    if files.is_empty() {
        eprintln!("error: no `.ql` files found under `{}`", path.display());
        return Err(1);
    }

    let mut has_errors = false;

    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            eprintln!("error: failed to read `{}`: {error}", file.display());
            1
        })?;

        match analyze_source(&source) {
            Ok(()) => println!("ok: {}", file.display()),
            Err(diagnostics) => {
                has_errors = true;
                print_diagnostics(&file, &source, &diagnostics);
            }
        }
    }

    if has_errors { Err(1) } else { Ok(()) }
}

fn format_path(path: &Path, write: bool) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match format_source(&source) {
        Ok(formatted) => {
            if write {
                fs::write(path, formatted).map_err(|error| {
                    eprintln!("error: failed to write `{}`: {error}", path.display());
                    1
                })?;
            } else {
                print!("{formatted}");
            }
            Ok(())
        }
        Err(errors) => {
            print_diagnostics(path, &source, &parse_errors_to_diagnostics(errors));
            Err(1)
        }
    }
}

fn render_mir_path(path: &Path) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match analyze_semantics(&source) {
        Ok(analysis) => {
            print!("{}", analysis.render_mir());
            if analysis.has_errors() {
                print_diagnostics(path, &source, analysis.diagnostics());
                Err(1)
            } else {
                Ok(())
            }
        }
        Err(diagnostics) => {
            print_diagnostics(path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn render_ownership_path(path: &Path) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match analyze_semantics(&source) {
        Ok(analysis) => {
            print!("{}", analysis.render_borrowck());
            if analysis.has_errors() {
                print_diagnostics(path, &source, analysis.diagnostics());
                Err(1)
            } else {
                Ok(())
            }
        }
        Err(diagnostics) => {
            print_diagnostics(path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn render_runtime_requirements_path(path: &Path) -> Result<(), u8> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read `{}`: {error}", path.display());
        1
    })?;

    match analyze_semantics(&source) {
        Ok(analysis) => {
            print!("{}", render_runtime_requirements(&analysis));
            if analysis.has_errors() {
                print_diagnostics(path, &source, analysis.diagnostics());
                Err(1)
            } else {
                Ok(())
            }
        }
        Err(diagnostics) => {
            print_diagnostics(path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn build_path(path: &Path, options: &BuildOptions) -> Result<(), u8> {
    match build_file(path, options) {
        Ok(artifact) => {
            println!(
                "wrote {}: {}",
                artifact.emit.as_str(),
                artifact.path.display()
            );
            if let Some(header) = artifact.c_header {
                println!("wrote c-header: {}", header.path.display());
            }
            Ok(())
        }
        Err(BuildError::InvalidInput(message)) => {
            eprintln!("error: {message}");
            Err(1)
        }
        Err(BuildError::Io { path, error }) => {
            eprintln!("error: failed to access `{}`: {error}", path.display());
            Err(1)
        }
        Err(BuildError::Toolchain {
            error,
            preserved_artifacts,
        }) => {
            eprintln!("error: {error}");
            for path in preserved_artifacts {
                eprintln!(
                    "note: preserved intermediate artifact at `{}`",
                    path.display()
                );
            }
            Err(1)
        }
        Err(BuildError::Diagnostics {
            path,
            source,
            diagnostics,
        }) => {
            print_diagnostics(&path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn emit_c_header_path(path: &Path, options: &CHeaderOptions) -> Result<(), u8> {
    match emit_c_header(path, options) {
        Ok(artifact) => {
            println!("wrote c-header: {}", artifact.path.display());
            Ok(())
        }
        Err(CHeaderError::InvalidInput(message)) => {
            eprintln!("error: {message}");
            Err(1)
        }
        Err(CHeaderError::Io { path, error }) => {
            eprintln!("error: failed to access `{}`: {error}", path.display());
            Err(1)
        }
        Err(CHeaderError::Diagnostics {
            path,
            source,
            diagnostics,
        }) => {
            print_diagnostics(&path, &source, &diagnostics);
            Err(1)
        }
    }
}

fn analyze_source(source: &str) -> Result<(), Vec<Diagnostic>> {
    let analysis = analyze_semantics(source)?;
    if analysis.has_errors() {
        Err(analysis.diagnostics().to_vec())
    } else {
        Ok(())
    }
}

fn render_runtime_requirements(analysis: &ql_analysis::Analysis) -> String {
    if analysis.runtime_requirements().is_empty() {
        return "runtime requirements: none\n".to_owned();
    }

    let mut rendered = String::new();
    for requirement in analysis.runtime_requirements() {
        rendered.push_str(&format!(
            "runtime requirement: {} @ {} ({})\n",
            requirement.capability.stable_name(),
            requirement.span,
            requirement.capability.description(),
        ));
    }
    for hook in collect_runtime_hooks(
        analysis
            .runtime_requirements()
            .iter()
            .map(|requirement| requirement.capability),
    ) {
        rendered.push_str(&format!(
            "runtime hook: {} -> {} ({})\n",
            hook.stable_name(),
            hook.symbol_name(),
            hook.description(),
        ));
    }
    rendered
}

fn collect_ql_files(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    collect_ql_files_recursive(path, path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_ql_files_recursive(
    root: &Path,
    path: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            if should_skip_directory(root, &entry_path) {
                continue;
            }
            collect_ql_files_recursive(root, &entry_path, files)?;
        } else if is_ql_file(&entry_path) && !should_skip_file(root, &entry_path) {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn is_ql_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("ql")
}

fn should_skip_directory(root: &Path, path: &Path) -> bool {
    if path == root {
        return false;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    name.starts_with('.')
        || matches!(
            name,
            "target" | "node_modules" | "dist" | "build" | "coverage" | "fixtures" | "ramdon_tests"
        )
        || is_negative_fixture_path(root, path)
}

fn should_skip_file(root: &Path, path: &Path) -> bool {
    if path == root {
        return false;
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
        || is_negative_fixture_path(root, path)
}

fn is_negative_fixture_path(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    let mut saw_fixtures = false;
    for component in relative.components().filter_map(component_name) {
        if component == "fixtures" {
            saw_fixtures = true;
            continue;
        }

        if saw_fixtures && component == "fail" {
            return true;
        }
    }

    false
}

fn component_name(component: Component<'_>) -> Option<&str> {
    match component {
        Component::Normal(segment) => segment.to_str(),
        _ => None,
    }
}

fn print_diagnostics(path: &Path, source: &str, diagnostics: &[Diagnostic]) {
    eprint!("{}", render_diagnostics(path, source, diagnostics));
}

fn print_usage() {
    eprintln!("Qlang CLI");
    eprintln!("usage:");
    eprintln!("  ql check <file-or-dir>");
    eprintln!(
        "  ql build <file> [--emit llvm-ir|obj|exe|dylib|staticlib] [--release] [-o <output>] [--header] [--header-surface exports|imports|both] [--header-output <output>]"
    );
    eprintln!("  ql ffi header <file> [--surface exports|imports|both] [-o <output>]");
    eprintln!("  ql fmt <file> [--write]");
    eprintln!("  ql mir <file>");
    eprintln!("  ql ownership <file>");
    eprintln!("  ql runtime <file>");
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ql_driver::{
        ArchiverFlavor, ArchiverInvocation, BuildEmit, BuildOptions, BuildProfile,
        ProgramInvocation, ToolchainOptions,
    };

    use super::{
        analyze_semantics, analyze_source, build_path, collect_ql_files, render_mir_path,
        render_ownership_path, render_runtime_requirements,
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

    fn relative_paths(root: &Path, files: Vec<PathBuf>) -> Vec<String> {
        files
            .into_iter()
            .map(|path| {
                path.strip_prefix(root)
                    .expect("file should be under test root")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn collect_ql_files_skips_tooling_and_negative_fixture_dirs() {
        let dir = TestDir::new("ql-cli-scan");
        dir.write("src/main.ql", "fn main() {}");
        dir.write("fixtures/parser/pass/good.ql", "fn good() {}");
        dir.write("fixtures/parser/fail/bad.ql", "fn");
        dir.write("ramdon_tests/scratch.ql", "fn scratch() {}");
        dir.write("target/generated.ql", "fn generated() {}");
        dir.write("node_modules/pkg/index.ql", "fn dep() {}");
        dir.write(".git/hooks/pre-commit.ql", "fn hook() {}");

        let files = collect_ql_files(dir.path()).expect("collect ql files");

        assert_eq!(relative_paths(dir.path(), files), vec!["src/main.ql"]);
    }

    #[test]
    fn collect_ql_files_respects_explicit_negative_fixture_roots() {
        let dir = TestDir::new("ql-cli-explicit-fail");
        dir.write("fixtures/parser/fail/bad.ql", "fn");

        let root = dir.path().join("fixtures/parser/fail");
        let files = collect_ql_files(&root).expect("collect explicit fail fixture files");

        assert_eq!(relative_paths(&root, files), vec!["bad.ql"]);
    }

    #[test]
    fn analyze_source_reports_semantic_errors() {
        let diagnostics = analyze_source(
            r#"
struct User {}
fn User() {}
"#,
        )
        .expect_err("source should have semantic diagnostics");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == "duplicate top-level definition `User`")
        );
    }

    #[test]
    fn analyze_source_reports_resolution_errors() {
        let diagnostics = analyze_source(
            r#"
fn main() -> Int {
    self
}
"#,
        )
        .expect_err("source should have resolver diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "invalid use of `self` outside a method receiver scope"
        }));
    }

    #[test]
    fn analyze_source_reports_type_errors() {
        let diagnostics = analyze_source(
            r#"
fn main() -> Int {
    return "oops"
}
"#,
        )
        .expect_err("source should have type diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "return value has type mismatch: expected `Int`, found `String`"
        }));
    }

    #[test]
    fn render_mir_path_succeeds_for_valid_sources() {
        let dir = TestDir::new("ql-cli-mir");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
        );

        assert!(render_mir_path(&dir.path().join("sample.ql")).is_ok());
    }

    #[test]
    fn render_ownership_path_surfaces_ownership_reports() {
        let dir = TestDir::new("ql-cli-ownership");
        dir.write(
            "sample.ql",
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#,
        );

        let result = render_ownership_path(&dir.path().join("sample.ql"));
        assert!(
            result.is_err(),
            "ownership diagnostics should fail the command"
        );
    }

    #[test]
    fn render_runtime_requirements_reports_async_surface() {
        let analysis = analyze_semantics(
            r#"
async fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    let task = spawn helper()
    return await helper()
}

async fn helper() -> Int {
    return 1
}
"#,
        )
        .expect("source should analyze");

        let rendered = render_runtime_requirements(&analysis);
        assert!(rendered.contains("runtime requirement: async-function-bodies @"));
        assert!(rendered.contains("runtime requirement: async-iteration @"));
        assert!(rendered.contains("runtime requirement: task-spawn @"));
        assert!(rendered.contains("runtime requirement: task-await @"));
        assert!(rendered.contains("runtime hook: async-task-create -> qlrt_async_task_create"));
        assert!(rendered.contains("runtime hook: executor-spawn -> qlrt_executor_spawn"));
        assert!(rendered.contains("runtime hook: task-await -> qlrt_task_await"));
        assert!(rendered.contains("runtime hook: async-iter-next -> qlrt_async_iter_next"));
    }

    #[test]
    fn render_runtime_requirements_reports_none_for_sync_sources() {
        let analysis = analyze_semantics(
            r#"
fn main() -> Int {
    return 1
}
"#,
        )
        .expect("source should analyze");

        assert_eq!(
            render_runtime_requirements(&analysis),
            "runtime requirements: none\n"
        );
    }

    #[test]
    fn build_path_emits_llvm_ir_for_supported_source() {
        let dir = TestDir::new("ql-cli-build");
        dir.write(
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

        assert!(build_path(&dir.path().join("sample.ql"), &options).is_ok());

        let rendered = fs::read_to_string(output).expect("read emitted LLVM IR");
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
    }

    #[test]
    fn build_path_emits_object_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-obj");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/sample.obj"
        } else {
            "artifacts/sample.o"
        });
        let options = BuildOptions {
            emit: BuildEmit::Object,
            profile: BuildProfile::Debug,
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        assert!(build_path(&dir.path().join("sample.ql"), &options).is_ok());

        let rendered = fs::read_to_string(output).expect("read emitted object placeholder");
        assert_eq!(rendered, "mock-object");
    }

    #[test]
    fn build_path_emits_executable_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-exe");
        dir.write(
            "sample.ql",
            r#"
fn main() -> Int {
    return 1
}
"#,
        );
        let output = dir.path().join(if cfg!(windows) {
            "artifacts/sample.exe"
        } else {
            "artifacts/sample"
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

        assert!(build_path(&dir.path().join("sample.ql"), &options).is_ok());

        let rendered = fs::read_to_string(output).expect("read emitted executable placeholder");
        assert_eq!(rendered, "mock-executable");
    }

    #[test]
    fn build_path_emits_dynamic_library_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-dylib");
        dir.write(
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
                clang: Some(mock_success_invocation(&dir)),
                ..ToolchainOptions::default()
            },
        };

        assert!(build_path(&dir.path().join("ffi_export.ql"), &options).is_ok());

        let rendered =
            fs::read_to_string(output).expect("read emitted dynamic library placeholder");
        assert_eq!(rendered, "mock-dylib");
    }

    #[test]
    fn build_path_emits_static_library_for_supported_source() {
        let dir = TestDir::new("ql-cli-build-staticlib");
        dir.write(
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
            output: Some(output.clone()),
            c_header: None,
            toolchain: ToolchainOptions {
                clang: Some(mock_success_invocation(&dir)),
                archiver: Some(mock_success_archiver_invocation(&dir)),
            },
        };

        assert!(build_path(&dir.path().join("math.ql"), &options).is_ok());

        let rendered = fs::read_to_string(output).expect("read emitted static library placeholder");
        assert_eq!(rendered, "mock-staticlib");
    }

    fn mock_success_invocation(dir: &TestDir) -> ProgramInvocation {
        if cfg!(windows) {
            let script = dir.write(
                "mock-clang-success.ps1",
                r#"
$out = $null
$isCompile = $false
$isShared = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-c') {
        $isCompile = $true
    }
    if ($args[$i] -eq '-shared' -or $args[$i] -eq '-dynamiclib') {
        $isShared = $true
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
}
