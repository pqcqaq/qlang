use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{SymbolKind, analyze_package, analyze_package_dependencies, analyze_source};
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
    let member_field = nth_offset(source, "value", 3);

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
    assert_eq!(references.len(), 3);
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
    assert_eq!(
        references[2].span,
        Span::new(member_field, member_field + "value".len())
    );
}

#[test]
fn package_analysis_exposes_dependency_value_queries_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-value-queries-parse-errors");
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
    let current = config
    return current.value
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_offset = nth_offset(source, "current", 2);

    let hover = package
        .dependency_value_hover_in_source_at(source, use_offset)
        .expect("dependency value hover should survive parse errors");
    assert_eq!(hover.kind, SymbolKind::Struct);
    assert_eq!(hover.name, "Config");
    assert!(hover.detail.starts_with("struct Config {"));

    let definition = package
        .dependency_value_definition_in_source_at(source, use_offset)
        .expect("dependency value definition should survive parse errors");
    assert_eq!(definition.kind, SymbolKind::Struct);
    assert_eq!(definition.name, "Config");
    assert!(definition.path.ends_with("dep.qi"));

    let type_definition = package
        .dependency_value_type_definition_in_source_at(source, use_offset)
        .expect("dependency value type definition should survive parse errors");
    assert_eq!(type_definition.kind, SymbolKind::Struct);
    assert_eq!(type_definition.name, "Config");
    assert_eq!(type_definition.path, definition.path);
    assert_eq!(type_definition.span, definition.span);
}

#[test]
fn package_analysis_exposes_dependency_import_call_member_queries_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-import-call-member-queries-parse-errors");
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

pub struct Child {
    value: Int,
}

impl Child {
    pub fn get(self) -> Int
}

pub fn load() -> Child
pub fn maybe_load() -> Option[Child]
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

use demo.dep.load
use demo.dep.maybe_load

pub fn read() -> Int {
    let first = load().value
    let second = load().get()
    let third = maybe_load()?.value
    let fourth = maybe_load()?.get()
    let fifth = load().value
    let sixth = maybe_load()?.get(
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let direct_field_hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "value", 1))
        .expect("dependency import call-result field hover should survive parse errors");
    assert_eq!(direct_field_hover.kind, SymbolKind::Field);
    assert_eq!(direct_field_hover.name, "value");
    assert_eq!(direct_field_hover.detail, "field value: Int");

    let direct_field_definition = package
        .dependency_struct_field_definition_in_source_at(source, nth_offset(source, "value", 2))
        .expect("dependency import call-result field definition should survive parse errors");
    assert_eq!(direct_field_definition.kind, SymbolKind::Field);
    assert_eq!(direct_field_definition.name, "value");
    assert!(direct_field_definition.path.ends_with("dep.qi"));

    let question_field_hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "value", 3))
        .expect("dependency import question-call field hover should survive parse errors");
    assert_eq!(question_field_hover.kind, SymbolKind::Field);
    assert_eq!(question_field_hover.name, "value");
    assert_eq!(question_field_hover.detail, "field value: Int");

    let direct_method_hover = package
        .dependency_method_hover_in_source_at(source, nth_offset(source, "get", 1))
        .expect("dependency import call-result method hover should survive parse errors");
    assert_eq!(direct_method_hover.kind, SymbolKind::Method);
    assert_eq!(direct_method_hover.name, "get");
    assert_eq!(direct_method_hover.detail, "fn get(self) -> Int");

    let direct_method_definition = package
        .dependency_method_definition_in_source_at(source, nth_offset(source, "get", 1))
        .expect("dependency import call-result method definition should survive parse errors");
    assert_eq!(direct_method_definition.kind, SymbolKind::Method);
    assert_eq!(direct_method_definition.name, "get");
    assert!(direct_method_definition.path.ends_with("dep.qi"));

    let question_method_hover = package
        .dependency_method_hover_in_source_at(source, nth_offset(source, "get", 2))
        .expect("dependency import question-call method hover should survive parse errors");
    assert_eq!(question_method_hover.kind, SymbolKind::Method);
    assert_eq!(question_method_hover.name, "get");
    assert_eq!(question_method_hover.detail, "fn get(self) -> Int");

    let field_references = package
        .dependency_struct_field_references_in_source_at(source, nth_offset(source, "value", 1))
        .expect("dependency import call-result field references should survive parse errors");
    assert_eq!(field_references.len(), 3);
    assert_eq!(
        field_references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "value", 1),
                nth_offset(source, "value", 1) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 2),
                nth_offset(source, "value", 2) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 3),
                nth_offset(source, "value", 3) + "value".len(),
            ),
        ]
    );

    let method_references = package
        .dependency_method_references_in_source_at(source, nth_offset(source, "get", 2))
        .expect("dependency import question-call method references should survive parse errors");
    assert_eq!(method_references.len(), 3);
    assert_eq!(
        method_references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "get", 1),
                nth_offset(source, "get", 1) + "get".len(),
            ),
            Span::new(
                nth_offset(source, "get", 2),
                nth_offset(source, "get", 2) + "get".len(),
            ),
            Span::new(
                nth_offset(source, "get", 3),
                nth_offset(source, "get", 3) + "get".len(),
            ),
        ]
    );
}

#[test]
fn package_analysis_exposes_dependency_direct_question_member_queries_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-direct-question-member-queries-parse-errors");
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

pub struct Child {
    value: Int,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {
    child: Option[Child],
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn get(self) -> Int
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
    let first = config.child?.value
    let second = config.child()?.get()
    let third = config.child?.value
    let fourth = config.child()?.get(
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field_hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "value", 1))
        .expect("dependency direct-question field hover should survive parse errors");
    assert_eq!(field_hover.kind, SymbolKind::Field);
    assert_eq!(field_hover.name, "value");
    assert_eq!(field_hover.detail, "field value: Int");

    let field_definition = package
        .dependency_struct_field_definition_in_source_at(source, nth_offset(source, "value", 2))
        .expect("dependency direct-question field definition should survive parse errors");
    assert_eq!(field_definition.kind, SymbolKind::Field);
    assert_eq!(field_definition.name, "value");
    assert!(field_definition.path.ends_with("dep.qi"));

    let method_hover = package
        .dependency_method_hover_in_source_at(source, nth_offset(source, "get", 1))
        .expect("dependency direct-question method hover should survive parse errors");
    assert_eq!(method_hover.kind, SymbolKind::Method);
    assert_eq!(method_hover.name, "get");
    assert_eq!(method_hover.detail, "fn get(self) -> Int");

    let method_definition = package
        .dependency_method_definition_in_source_at(source, nth_offset(source, "get", 2))
        .expect("dependency direct-question method definition should survive parse errors");
    assert_eq!(method_definition.kind, SymbolKind::Method);
    assert_eq!(method_definition.name, "get");
    assert!(method_definition.path.ends_with("dep.qi"));

    let field_references = package
        .dependency_struct_field_references_in_source_at(source, nth_offset(source, "value", 1))
        .expect("dependency direct-question field references should survive parse errors");
    assert_eq!(
        field_references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "value", 1),
                nth_offset(source, "value", 1) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 2),
                nth_offset(source, "value", 2) + "value".len(),
            ),
        ]
    );

    let method_references = package
        .dependency_method_references_in_source_at(source, nth_offset(source, "get", 1))
        .expect("dependency direct-question method references should survive parse errors");
    assert_eq!(
        method_references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "get", 1),
                nth_offset(source, "get", 1) + "get".len(),
            ),
            Span::new(
                nth_offset(source, "get", 2),
                nth_offset(source, "get", 2) + "get".len(),
            ),
        ]
    );
}

#[test]
fn package_analysis_exposes_dependency_direct_question_member_type_definitions_in_broken_source() {
    let temp =
        TempDir::new("ql-analysis-package-direct-question-member-type-definitions-parse-errors");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {
    child: Option[Child],
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn leaf(self) -> Leaf
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
    let field = config.child?.leaf
    let method = config.child()?.leaf()
    let broken = config.child()?.leaf(
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field_type = package
        .dependency_struct_field_type_definition_in_source_at(source, nth_offset(source, "leaf", 1))
        .expect("dependency direct-question field type definition should survive parse errors");
    assert_eq!(field_type.kind, SymbolKind::Struct);
    assert_eq!(field_type.name, "Leaf");
    assert!(field_type.path.ends_with("dep.qi"));

    let method_type = package
        .dependency_method_type_definition_in_source_at(source, nth_offset(source, "leaf", 2))
        .expect("dependency direct-question method type definition should survive parse errors");
    assert_eq!(method_type.kind, SymbolKind::Struct);
    assert_eq!(method_type.name, "Leaf");
    assert_eq!(method_type.path, field_type.path);
    assert_eq!(method_type.span, field_type.span);
}

#[test]
fn package_analysis_exposes_dependency_direct_indexed_receiver_field_queries_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-direct-indexed-receiver-field-parse-errors");
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

pub struct Child {
    value: Int,
}

pub struct Config {
    children: [Child; 2],
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
    let first = config.children[0].value
    let second = config.children[1].value
    let broken = config.children[0].value
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "value", 1))
        .expect("dependency direct-indexed field hover should survive parse errors");
    assert_eq!(hover.kind, SymbolKind::Field);
    assert_eq!(hover.name, "value");
    assert_eq!(hover.detail, "field value: Int");

    let definition = package
        .dependency_struct_field_definition_in_source_at(source, nth_offset(source, "value", 2))
        .expect("dependency direct-indexed field definition should survive parse errors");
    assert_eq!(definition.kind, SymbolKind::Field);
    assert_eq!(definition.name, "value");
    assert!(definition.path.ends_with("dep.qi"));

    let references = package
        .dependency_struct_field_references_in_source_at(source, nth_offset(source, "value", 3))
        .expect("dependency direct-indexed field references should survive parse errors");
    assert_eq!(references.len(), 3);
    assert_eq!(references[0].name, "value");
    assert_eq!(
        references[0].span,
        Span::new(
            nth_offset(source, "value", 1),
            nth_offset(source, "value", 1) + "value".len(),
        )
    );
    assert_eq!(
        references[1].span,
        Span::new(
            nth_offset(source, "value", 2),
            nth_offset(source, "value", 2) + "value".len(),
        )
    );
    assert_eq!(
        references[2].span,
        Span::new(
            nth_offset(source, "value", 3),
            nth_offset(source, "value", 3) + "value".len(),
        )
    );
}

#[test]
fn package_analysis_exposes_dependency_root_indexed_receiver_field_queries_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-root-indexed-receiver-field-parse-errors");
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

pub struct Child {
    value: Int,
}

pub fn load_children() -> [Child; 2]
pub fn maybe_children() -> Option[[Child; 2]]
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

use demo.dep.load_children
use demo.dep.maybe_children

pub fn read() -> Int {
    let first = load_children()[0].value
    let second = maybe_children()?[1].value
    let third = load_children()[1].value
    let fourth = maybe_children()?[0].value
    let broken = maybe_children()?[0].value
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let direct_hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "value", 1))
        .expect("dependency root-indexed call field hover should survive parse errors");
    assert_eq!(direct_hover.kind, SymbolKind::Field);
    assert_eq!(direct_hover.name, "value");
    assert_eq!(direct_hover.detail, "field value: Int");

    let question_hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "value", 2))
        .expect("dependency root-indexed question-call field hover should survive parse errors");
    assert_eq!(question_hover.kind, SymbolKind::Field);
    assert_eq!(question_hover.name, "value");
    assert_eq!(question_hover.detail, "field value: Int");

    let definition = package
        .dependency_struct_field_definition_in_source_at(source, nth_offset(source, "value", 3))
        .expect("dependency root-indexed call field definition should survive parse errors");
    assert_eq!(definition.kind, SymbolKind::Field);
    assert_eq!(definition.name, "value");
    assert!(definition.path.ends_with("dep.qi"));

    let references = package
        .dependency_struct_field_references_in_source_at(source, nth_offset(source, "value", 5))
        .expect("dependency root-indexed field references should survive parse errors");
    assert_eq!(references.len(), 5);
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "value", 1),
                nth_offset(source, "value", 1) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 2),
                nth_offset(source, "value", 2) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 3),
                nth_offset(source, "value", 3) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 4),
                nth_offset(source, "value", 4) + "value".len(),
            ),
            Span::new(
                nth_offset(source, "value", 5),
                nth_offset(source, "value", 5) + "value".len(),
            ),
        ]
    );
}

#[test]
fn package_analysis_exposes_dependency_root_indexed_value_queries_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-root-indexed-value-queries-parse-errors");
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

pub struct Child {
    value: Int,
}

pub fn load_children() -> [Child; 2]
pub fn maybe_children() -> Option[[Child; 2]]
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

use demo.dep.load_children
use demo.dep.maybe_children

pub fn read() -> Int {
    let first = load_children()[0]
    let second = maybe_children()?[1]
    return first.value + second.value + first.value
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let direct_hover = package
        .dependency_value_hover_in_source_at(source, nth_offset(source, "first", 2))
        .expect("dependency root-indexed value hover should survive parse errors");
    assert_eq!(direct_hover.kind, SymbolKind::Struct);
    assert_eq!(direct_hover.name, "Child");
    assert!(direct_hover.detail.starts_with("struct Child {"));

    let question_definition = package
        .dependency_value_definition_in_source_at(source, nth_offset(source, "second", 2))
        .expect("dependency root-indexed question value definition should survive parse errors");
    assert_eq!(question_definition.kind, SymbolKind::Struct);
    assert_eq!(question_definition.name, "Child");
    assert!(question_definition.path.ends_with("dep.qi"));

    let question_type_definition = package
        .dependency_value_type_definition_in_source_at(source, nth_offset(source, "second", 2))
        .expect(
            "dependency root-indexed question value type definition should survive parse errors",
        );
    assert_eq!(question_type_definition.kind, SymbolKind::Struct);
    assert_eq!(question_type_definition.name, "Child");
    assert_eq!(question_type_definition.path, question_definition.path);
    assert_eq!(question_type_definition.span, question_definition.span);

    let references = package
        .dependency_value_references_in_source_at(source, nth_offset(source, "first", 1))
        .expect("dependency root-indexed value references should survive parse errors");
    assert_eq!(references.len(), 3);
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "first", 1),
                nth_offset(source, "first", 1) + "first".len(),
            ),
            Span::new(
                nth_offset(source, "first", 2),
                nth_offset(source, "first", 2) + "first".len(),
            ),
            Span::new(
                nth_offset(source, "first", 3),
                nth_offset(source, "first", 3) + "first".len(),
            ),
        ]
    );
}

#[test]
fn package_analysis_exposes_dependency_structured_root_indexed_value_queries_in_broken_source() {
    let temp =
        TempDir::new("ql-analysis-package-structured-root-indexed-value-queries-parse-errors");
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

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]
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

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0]
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1]
    return first.value + second.value + first.value
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let direct_hover = package
        .dependency_value_hover_in_source_at(source, nth_offset(source, "first", 2))
        .expect("dependency structured root-indexed value hover should survive parse errors");
    assert_eq!(direct_hover.kind, SymbolKind::Struct);
    assert_eq!(direct_hover.name, "Child");
    assert!(direct_hover.detail.starts_with("struct Child {"));

    let match_definition = package
        .dependency_value_definition_in_source_at(source, nth_offset(source, "second", 2))
        .expect(
            "dependency structured match root-indexed value definition should survive parse errors",
        );
    assert_eq!(match_definition.kind, SymbolKind::Struct);
    assert_eq!(match_definition.name, "Child");
    assert!(match_definition.path.ends_with("dep.qi"));

    let match_type_definition = package
        .dependency_value_type_definition_in_source_at(source, nth_offset(source, "second", 2))
        .expect(
            "dependency structured match root-indexed value type definition should survive parse errors",
        );
    assert_eq!(match_type_definition.kind, SymbolKind::Struct);
    assert_eq!(match_type_definition.name, "Child");
    assert_eq!(match_type_definition.path, match_definition.path);
    assert_eq!(match_type_definition.span, match_definition.span);

    let references = package
        .dependency_value_references_in_source_at(source, nth_offset(source, "first", 1))
        .expect("dependency structured root-indexed value references should survive parse errors");
    assert_eq!(references.len(), 3);
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "first", 1),
                nth_offset(source, "first", 1) + "first".len(),
            ),
            Span::new(
                nth_offset(source, "first", 2),
                nth_offset(source, "first", 2) + "first".len(),
            ),
            Span::new(
                nth_offset(source, "first", 3),
                nth_offset(source, "first", 3) + "first".len(),
            ),
        ]
    );
}

#[test]
fn package_analysis_exposes_dependency_structured_root_indexed_member_queries_in_broken_source() {
    let temp =
        TempDir::new("ql-analysis-package-structured-root-indexed-member-queries-parse-errors");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

impl Child {
    pub fn leaf(self) -> Leaf
}

pub fn maybe_children() -> Option[[Child; 2]]
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

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0].leaf.value
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1].leaf()
    let third = (if flag { maybe_children()? } else { maybe_children()? })[1].leaf.value
    let broken = (match flag { true => maybe_children()?, false => maybe_children()? })[0].leaf(
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field_hover = package
        .dependency_struct_field_hover_in_source_at(source, nth_offset(source, "leaf", 1))
        .expect("dependency structured root-indexed field hover should survive parse errors");
    assert_eq!(field_hover.kind, SymbolKind::Field);
    assert_eq!(field_hover.name, "leaf");
    assert_eq!(field_hover.detail, "field leaf: Leaf");

    let field_definition = package
        .dependency_struct_field_definition_in_source_at(source, nth_offset(source, "leaf", 3))
        .expect("dependency structured root-indexed field definition should survive parse errors");
    assert_eq!(field_definition.kind, SymbolKind::Field);
    assert_eq!(field_definition.name, "leaf");
    assert!(field_definition.path.ends_with("dep.qi"));

    let method_hover = package
        .dependency_method_hover_in_source_at(source, nth_offset(source, "leaf", 2))
        .expect("dependency structured root-indexed method hover should survive parse errors");
    assert_eq!(method_hover.kind, SymbolKind::Method);
    assert_eq!(method_hover.name, "leaf");
    assert_eq!(method_hover.detail, "fn leaf(self) -> Leaf");

    let method_definition = package
        .dependency_method_definition_in_source_at(source, nth_offset(source, "leaf", 4))
        .expect("dependency structured root-indexed method definition should survive parse errors");
    assert_eq!(method_definition.kind, SymbolKind::Method);
    assert_eq!(method_definition.name, "leaf");
    assert!(method_definition.path.ends_with("dep.qi"));

    let field_references = package
        .dependency_struct_field_references_in_source_at(source, nth_offset(source, "leaf", 1))
        .expect("dependency structured root-indexed field references should survive parse errors");
    assert_eq!(
        field_references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "leaf", 1),
                nth_offset(source, "leaf", 1) + "leaf".len(),
            ),
            Span::new(
                nth_offset(source, "leaf", 3),
                nth_offset(source, "leaf", 3) + "leaf".len(),
            ),
        ]
    );

    let method_references = package
        .dependency_method_references_in_source_at(source, nth_offset(source, "leaf", 2))
        .expect("dependency structured root-indexed method references should survive parse errors");
    assert_eq!(
        method_references
            .iter()
            .map(|reference| reference.span)
            .collect::<Vec<_>>(),
        vec![
            Span::new(
                nth_offset(source, "leaf", 2),
                nth_offset(source, "leaf", 2) + "leaf".len(),
            ),
            Span::new(
                nth_offset(source, "leaf", 4),
                nth_offset(source, "leaf", 4) + "leaf".len(),
            ),
        ]
    );
}
