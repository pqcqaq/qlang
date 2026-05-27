use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn direct_bridge_items_return_empty_without_direct_dependencies() {
    let dir = TestDir::new("ql-direct-bridge-no-dependencies");
    let manifest_path = dir.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let source = "fn main() -> Int { return 0 }\n";

    let items = render_direct_dependency_bridge_items("ql build", &manifest_path, source, false)
        .expect("project without direct dependencies should not render bridge items");

    assert!(items.declarations.is_empty());
    assert!(items.source_rewrites.is_empty());
}
