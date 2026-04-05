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

fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, _)| start)
        .expect("needle occurrence should exist")
}

#[test]
fn package_analysis_surfaces_dependency_import_completions() {
    let temp = TempDir::new("ql-analysis-package-completion");
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

pub fn exported(value: Int) -> Int
pub struct Buffer[T] {
    value: T,
}
pub const DEFAULT_PORT: Int
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
    let source = r#"
package demo.app

use demo.dep.Bu

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_completions_at(source, nth_offset(source, "Bu", 1) + "Bu".len())
        .expect("dependency completions should exist");

    assert!(completions.iter().any(|item| {
        item.label == "Buffer"
            && item.kind == SymbolKind::Struct
            && item.detail.starts_with("struct Buffer[T] {")
    }));
    assert!(completions.iter().any(|item| {
        item.label == "DEFAULT_PORT"
            && item.kind == SymbolKind::Const
            && item.detail == "const DEFAULT_PORT: Int"
    }));
}

#[test]
fn package_analysis_surfaces_dependency_import_path_segment_completions() {
    let temp = TempDir::new("ql-analysis-package-path-completion");
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

pub struct Buffer[T] {
    value: T,
}
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
    let source = r#"
package demo.app

use demo.d

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_completions_at(source, nth_offset(source, "demo.d", 1) + "demo.d".len())
        .expect("dependency path completions should exist");

    assert!(completions.iter().any(|item| {
        item.label == "dep" && item.kind == SymbolKind::Import && item.detail == "package demo.dep"
    }));
}

#[test]
fn package_analysis_surfaces_grouped_dependency_import_completions() {
    let temp = TempDir::new("ql-analysis-grouped-package-completion");
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

pub fn exported(value: Int) -> Int
pub struct Buffer[T] {
    value: T,
}
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
    let source = r#"
package demo.app

use demo.dep.{exported as run, Bu}

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_completions_at(source, nth_offset(source, "Bu", 1) + "Bu".len())
        .expect("grouped dependency completions should exist");

    assert!(completions.iter().any(|item| {
        item.label == "Buffer"
            && item.kind == SymbolKind::Struct
            && item.detail.starts_with("struct Buffer[T] {")
    }));
}
