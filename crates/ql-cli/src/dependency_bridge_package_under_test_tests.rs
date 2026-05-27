use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ql_project::{BuildTarget, BuildTargetKind, ManifestBuildProfile};

use super::*;

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
fn package_under_test_bridge_returns_empty_for_unselected_member() {
    let dir = TestDir::new("ql-package-under-test-bridge-unselected");
    let manifest_path = dir.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let source = "use app.add as add\n\nfn main() -> Int { return add(1) }\n";

    let items =
        render_package_under_test_bridge_items("ql test", &[], &manifest_path, source, false)
            .expect("unselected package should not fail bridge rendering");

    assert!(items.declarations.is_empty());
    assert!(items.source_rewrites.is_empty());
}

#[test]
fn package_under_test_bridge_includes_imported_public_function_forwarders() {
    let dir = TestDir::new("ql-package-under-test-bridge-function");
    let manifest_path = dir.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let lib_path = dir.write(
        "app/src/lib.ql",
        "pub fn add(value: Int) -> Int { return value + 1 }\n",
    );
    let workspace_members = vec![WorkspaceBuildTargets {
        member_manifest_path: manifest_path.clone(),
        package_name: "app".to_owned(),
        default_profile: Some(ManifestBuildProfile::Debug),
        targets: vec![BuildTarget {
            kind: BuildTargetKind::Library,
            path: lib_path,
        }],
    }];
    let source = "use app.add as add\n\nfn main() -> Int { return add(1) }\n";

    let items = render_package_under_test_bridge_items(
        "ql test",
        &workspace_members,
        &manifest_path,
        source,
        false,
    )
    .expect("package-under-test public function bridge should render");

    assert!(
        items
            .declarations
            .contains("extern \"c\" fn __ql_bridge_app_add")
    );
    assert!(items.declarations.contains("const add: (Int) -> Int"));
    assert!(items.source_rewrites.is_empty());
}
