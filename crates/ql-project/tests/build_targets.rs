use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_project::{
    BuildTargetKind, ProjectError, discover_package_build_targets, load_project_manifest,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
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
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn discover_package_build_targets_prefers_declared_lib_and_bin_paths() {
    let temp = TempDir::new("ql-project-build-targets-declared");
    let app_root = temp.path().join("app");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[lib]
path = "src/runtime/core.ql"

[[bin]]
path = "src/tools/repl.ql"
"#,
    );
    temp.write(
        "app/src/runtime/core.ql",
        "pub fn core() -> Int { return 1 }\n",
    );
    temp.write("app/src/tools/repl.ql", "fn main() -> Int { return 0 }\n");
    temp.write(
        "app/src/lib.ql",
        "pub fn default_lib() -> Int { return 2 }\n",
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 3 }\n");

    let manifest = load_project_manifest(&app_root).expect("load app manifest");
    let targets = discover_package_build_targets(&manifest).expect("discover declared targets");
    let relative_targets = targets
        .into_iter()
        .map(|target| {
            (
                target.kind,
                target
                    .path
                    .strip_prefix(&app_root)
                    .expect("target should stay inside package root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        relative_targets,
        vec![
            (BuildTargetKind::Library, "src/runtime/core.ql".to_owned()),
            (BuildTargetKind::Binary, "src/tools/repl.ql".to_owned()),
        ],
        "explicit target declarations should override default convention-based entry discovery"
    );
}

#[test]
fn discover_package_build_targets_rejects_declared_targets_outside_src() {
    let temp = TempDir::new("ql-project-build-targets-outside-src");
    let app_root = temp.path().join("app");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[lib]
path = "../shared/core.ql"
"#,
    );
    temp.write("shared/core.ql", "pub fn core() -> Int { return 1 }\n");
    fs::create_dir_all(app_root.join("src")).expect("create package source root");

    let manifest = load_project_manifest(&app_root).expect("load app manifest");
    let error = discover_package_build_targets(&manifest)
        .expect_err("declared targets outside `src/` should be rejected");
    let ProjectError::Parse { message, .. } = error else {
        panic!("declared target validation should surface a manifest parse error");
    };
    assert!(
        message.contains("must stay under `src/`"),
        "expected out-of-source target rejection, got: {message}"
    );
}
