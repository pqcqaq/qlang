use std::path::{Path, PathBuf};

use crate::cli_scan::collect_ql_files;
use crate::cli_utils::normalize_path;

pub(super) fn collect_project_test_files(package_root: &Path) -> Result<Vec<PathBuf>, u8> {
    let tests_root = project_tests_root(package_root);
    if !tests_root.is_dir() {
        return Ok(Vec::new());
    }

    collect_ql_files(&tests_root).map_err(|error| {
        eprintln!(
            "{}",
            project_test_files_read_error_message(&tests_root, error)
        );
        1
    })
}

fn project_tests_root(package_root: &Path) -> PathBuf {
    package_root.join("tests")
}

fn project_test_files_read_error_message(
    tests_root: &Path,
    error: impl std::fmt::Display,
) -> String {
    format!(
        "error: `ql test` failed to read `{}`: {error}",
        normalize_path(tests_root)
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{collect_project_test_files, project_test_files_read_error_message};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
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
    fn project_test_files_are_empty_without_tests_directory() {
        let dir = TestDir::new("ql-project-test-files-empty");

        let files =
            collect_project_test_files(dir.path()).expect("collect missing tests directory");

        assert!(files.is_empty());
    }

    #[test]
    fn project_test_files_collect_only_tests_directory_ql_sources() {
        let dir = TestDir::new("ql-project-test-files");
        dir.write("src/main.ql", "fn main() {}");
        dir.write("tests/z_smoke.ql", "fn main() -> Int { return 0 }");
        dir.write("tests/api/basic.ql", "fn main() -> Int { return 0 }");
        dir.write("tests/readme.txt", "not qlang");

        let files = collect_project_test_files(dir.path()).expect("collect package tests");

        assert_eq!(
            relative_paths(dir.path(), files),
            vec!["tests/api/basic.ql", "tests/z_smoke.ql"]
        );
    }

    #[test]
    fn project_test_files_read_error_message_names_tests_root() {
        let message = project_test_files_read_error_message(Path::new("pkg/tests"), "boom");

        assert_eq!(message, "error: `ql test` failed to read `pkg/tests`: boom");
    }
}
