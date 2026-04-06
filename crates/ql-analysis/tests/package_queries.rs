use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{SymbolKind, analyze_package, analyze_source};
use ql_span::Span;

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

#[test]
fn package_analysis_exposes_grouped_dependency_target_through_imports() {
    let temp = TempDir::new("ql-analysis-grouped-package-queries");
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

use demo.dep.{exported as run}

pub fn main() -> Int {
    return run(1)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let target = package
        .dependency_target_at(&analysis, nth_offset(source, "run", 1))
        .expect("grouped dependency target should exist");
    assert_eq!(target.kind, SymbolKind::Function);
    assert_eq!(target.name, "exported");
    assert_eq!(target.detail, "fn exported(value: Int) -> Int");
    assert_eq!(
        source
            .get(target.import_span.start..target.import_span.end)
            .expect("import span should slice source"),
        "run",
    );

    let artifact = fs::read_to_string(&target.path)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = artifact
        .get(target.definition_span.start..target.definition_span.end)
        .expect("definition span should slice artifact text");
    assert_eq!(snippet.trim(), "fn exported(value: Int) -> Int");
}

#[test]
fn package_analysis_exposes_dependency_variant_hover_and_definition() {
    let temp = TempDir::new("ql-analysis-package-variant-queries");
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

pub enum Command {
    Retry(Int),
    Stop,
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

use demo.dep.Command as Cmd

pub fn main() -> Int {
    let value = Cmd.Retry(1)
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let hover = package
        .dependency_variant_hover_at(&analysis, source, nth_offset(source, "Retry", 1))
        .expect("dependency variant hover should exist");
    assert_eq!(hover.kind, SymbolKind::Variant);
    assert_eq!(hover.name, "Retry");
    assert_eq!(hover.detail, "variant Command.Retry(Int)");
    assert_eq!(
        source
            .get(hover.span.start..hover.span.end)
            .expect("hover span should slice source"),
        "Retry",
    );

    let definition = package
        .dependency_variant_definition_at(&analysis, source, nth_offset(source, "Retry", 1))
        .expect("dependency variant definition should exist");
    assert_eq!(definition.kind, SymbolKind::Variant);
    assert_eq!(definition.name, "Retry");
    assert!(definition.path.ends_with("dep.qi"));

    let artifact = fs::read_to_string(&definition.path)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = artifact
        .get(definition.span.start..definition.span.end)
        .expect("definition span should slice artifact text");
    assert_eq!(snippet.trim(), "Retry");
}

#[test]
fn package_analysis_exposes_dependency_variant_references() {
    let temp = TempDir::new("ql-analysis-package-variant-references");
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

pub enum Command {
    Retry(Int),
    Stop,
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

use demo.dep.Command as Cmd

pub fn first() -> Int {
    let value = Cmd.Retry(1)
    return 0
}

pub fn second() -> Int {
    let value = Cmd.Retry(2)
    return 1
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let references = package
        .dependency_variant_references_at(&analysis, source, nth_offset(source, "Retry", 2))
        .expect("dependency variant references should exist");
    assert_eq!(references.len(), 2);
    assert!(
        references
            .iter()
            .all(|reference| reference.kind == SymbolKind::Variant)
    );
    assert!(references.iter().all(|reference| !reference.is_definition));
    assert_eq!(references[0].name, "Retry");
    assert_eq!(
        references[0].span,
        Span::new(
            nth_offset(source, "Retry", 1),
            nth_offset(source, "Retry", 1) + "Retry".len(),
        )
    );
    assert_eq!(
        references[1].span,
        Span::new(
            nth_offset(source, "Retry", 2),
            nth_offset(source, "Retry", 2) + "Retry".len(),
        )
    );
}

#[test]
fn package_analysis_exposes_dependency_struct_field_queries() {
    let temp = TempDir::new("ql-analysis-package-struct-field-queries");
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

pub struct Config {
    value: Int,
    limit: Int,
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

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    match config {
        Cfg { value: current, limit: 3 } => current,
    }
    return built.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_field = nth_offset(source, "value", 1);
    let pattern_field = nth_offset(source, "value", 2);

    let hover = package
        .dependency_struct_field_hover_at(&analysis, literal_field)
        .expect("dependency struct field hover should exist");
    assert_eq!(hover.kind, SymbolKind::Field);
    assert_eq!(hover.name, "value");
    assert_eq!(hover.detail, "field value: Int");
    assert_eq!(
        source
            .get(hover.span.start..hover.span.end)
            .expect("hover span should slice source"),
        "value",
    );

    let definition = package
        .dependency_struct_field_definition_at(&analysis, pattern_field)
        .expect("dependency struct field definition should exist");
    assert_eq!(definition.kind, SymbolKind::Field);
    assert_eq!(definition.name, "value");
    assert!(definition.path.ends_with("dep.qi"));

    let artifact = fs::read_to_string(&definition.path)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = artifact
        .get(definition.span.start..definition.span.end)
        .expect("definition span should slice artifact text");
    assert_eq!(snippet.trim(), "value");

    let references = package
        .dependency_struct_field_references_at(&analysis, literal_field)
        .expect("dependency struct field references should exist");
    assert_eq!(references.len(), 2);
    assert!(
        references
            .iter()
            .all(|reference| reference.kind == SymbolKind::Field)
    );
    assert!(references.iter().all(|reference| !reference.is_definition));
    assert_eq!(references[0].name, "value");
    assert_eq!(
        references[0].span,
        Span::new(literal_field, literal_field + "value".len())
    );
    assert_eq!(
        references[1].span,
        Span::new(pattern_field, pattern_field + "value".len())
    );
}
