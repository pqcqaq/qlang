use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{SymbolKind, analyze_package, analyze_source};

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

fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, _)| start)
        .expect("needle occurrence should exist")
}

#[test]
fn package_analysis_exposes_dependency_hover_and_definition_through_imports() {
    let temp = TempDir::new("ql-analysis-package-queries");
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

use demo.dep.exported as run
use demo.dep.Buffer as Buf

pub fn main(value: Buf[Int]) -> Int {
    return run(1)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let hover = package
        .dependency_hover_at(&analysis, nth_offset(source, "Buf", 2))
        .expect("dependency hover should exist");
    assert_eq!(hover.kind, SymbolKind::Struct);
    assert_eq!(hover.name, "Buffer");
    assert!(hover.detail.starts_with("struct Buffer[T] {"));
    assert_eq!(hover.source_path, "src/lib.ql");

    let definition = package
        .dependency_definition_at(&analysis, nth_offset(source, "run", 2))
        .expect("dependency definition should exist");
    assert_eq!(definition.kind, SymbolKind::Function);
    assert_eq!(definition.name, "exported");
    assert!(definition.path.ends_with("dep.qi"));

    let artifact = fs::read_to_string(&definition.path)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = artifact
        .get(definition.span.start..definition.span.end)
        .expect("definition span should slice artifact text");
    assert_eq!(snippet.trim(), "fn exported(value: Int) -> Int");
}
