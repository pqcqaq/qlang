use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use ql_analysis::{analyze_source as analyze_semantics, parse_errors_to_diagnostics};
use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_fmt::format_source;

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

fn analyze_source(source: &str) -> Result<(), Vec<Diagnostic>> {
    let analysis = analyze_semantics(source)?;
    if analysis.has_errors() {
        Err(analysis.diagnostics().to_vec())
    } else {
        Ok(())
    }
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
    eprintln!("  ql fmt <file> [--write]");
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{analyze_source, collect_ql_files};

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

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create test parent directory");
            }
            fs::write(path, contents).expect("write test file");
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
}
