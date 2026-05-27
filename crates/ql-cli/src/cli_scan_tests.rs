use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli_scan::collect_ql_files;

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
