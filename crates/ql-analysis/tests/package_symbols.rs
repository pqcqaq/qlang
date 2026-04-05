use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{SymbolKind, analyze_package};

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

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(path, contents).expect("write temp file");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn analyze_package_indexes_dependency_interface_symbols() {
    let temp = TempDir::new("ql-analysis-package-symbols");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub const DEFAULT_PORT: Int
pub static BUILD_ID: Int

pub fn exported(value: Int) -> Int

pub struct Buffer[T] {
    value: T,
}

pub enum Mode {
    Ready,
}

pub trait Reader {
    fn read(self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int
}

extend Buffer[Int] {
    pub fn twice(self) -> Int
}

pub type Port = Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let analysis = analyze_package(&app_root).expect("package analysis should succeed");
    let symbols = analysis.dependency_symbols();

    assert_eq!(symbols.len(), 10);
    assert!(symbols.iter().all(|symbol| symbol.package_name == "dep"));
    assert!(
        symbols
            .iter()
            .all(|symbol| symbol.source_path == "src/lib.ql")
    );

    let exported = analysis.dependency_symbols_named("exported");
    assert_eq!(exported.len(), 1);
    assert_eq!(exported[0].kind, SymbolKind::Function);
    assert_eq!(exported[0].detail, "fn exported(value: Int) -> Int");

    let len = analysis.dependency_symbols_named("len");
    assert_eq!(len.len(), 1);
    assert_eq!(len[0].kind, SymbolKind::Method);
    assert_eq!(len[0].detail, "fn len(self) -> Int");

    let read = analysis.dependency_symbols_named("read");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].kind, SymbolKind::Method);
    assert_eq!(read[0].detail, "fn read(self) -> Int");

    let buffer = analysis.dependency_symbols_named("Buffer");
    assert_eq!(buffer.len(), 1);
    assert_eq!(buffer[0].kind, SymbolKind::Struct);
    assert!(buffer[0].detail.starts_with("struct Buffer[T] {"));
    assert!(buffer[0].detail.contains("value: T,"));

    let reader = analysis.dependency_symbols_named("Reader");
    assert_eq!(reader.len(), 1);
    assert_eq!(reader[0].kind, SymbolKind::Trait);
    assert!(reader[0].detail.starts_with("trait Reader {"));

    let port = analysis.dependency_symbols_named("Port");
    assert_eq!(port.len(), 1);
    assert_eq!(port[0].kind, SymbolKind::TypeAlias);
    assert_eq!(port[0].detail, "type Port = Int");
}
