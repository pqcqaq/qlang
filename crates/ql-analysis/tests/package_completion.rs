use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{SymbolKind, analyze_package, analyze_package_dependencies};

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

#[test]
fn package_analysis_grouped_dependency_completion_skips_existing_items() {
    let temp = TempDir::new("ql-analysis-grouped-package-dedup");
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

use demo.dep.{exported, }

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_completions_at(source, nth_offset(source, ", }", 1) + 2)
        .expect("grouped dependency completions should exist");

    assert!(completions.iter().any(|item| item.label == "Buffer"));
    assert!(!completions.iter().any(|item| item.label == "exported"));
}

#[test]
fn package_analysis_surfaces_dependency_variant_completions_through_import_alias() {
    let temp = TempDir::new("ql-analysis-package-variant-completion");
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
    return Cmd.Re()
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_variant_completions_at(source, nth_offset(source, ".Re", 1) + 3)
        .expect("dependency variant completions should exist");

    assert_eq!(
        completions
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Retry", "Stop"]
    );
    assert!(
        completions
            .iter()
            .all(|item| item.kind == SymbolKind::Variant)
    );
    assert!(
        completions
            .iter()
            .any(|item| { item.label == "Retry" && item.detail == "variant Command.Retry(Int)" })
    );
}

#[test]
fn package_analysis_surfaces_dependency_struct_field_completions_from_parse_only_contexts() {
    let temp = TempDir::new("ql-analysis-package-struct-field-completion");
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
    flag: Bool,
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

pub fn main(current: Int, built: Cfg) -> Int {
    let next = Cfg { value: current, fl: true }
    let Cfg { value: reused, fl: enabled } = built
    return missing
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let literal = package
        .dependency_struct_field_completions_at(source, nth_offset(source, "fl", 1) + "fl".len())
        .expect("struct literal field completions should exist");
    assert_eq!(
        literal
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["flag"]
    );
    assert!(literal.iter().all(|item| item.kind == SymbolKind::Field));
    assert_eq!(literal[0].detail, "field flag: Bool");

    let pattern = package
        .dependency_struct_field_completions_at(source, nth_offset(source, "fl", 2) + "fl".len())
        .expect("struct pattern field completions should exist");
    assert_eq!(
        pattern
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["flag"]
    );
    assert!(pattern.iter().all(|item| item.kind == SymbolKind::Field));
}

#[test]
fn package_analysis_surfaces_dependency_value_root_member_completions_in_parse_error_source() {
    let temp = TempDir::new("ql-analysis-package-value-root-member-completion-parse-error");
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

impl Config {
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
    let current = config
    return current.va + current.ge(
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field = package
        .dependency_member_field_completions_at(source, nth_offset(source, ".va", 1) + ".va".len())
        .expect("dependency value-root field completions should survive parse errors");
    assert_eq!(
        field
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["value"]
    );
    assert!(field.iter().all(|item| item.kind == SymbolKind::Field));
    assert_eq!(field[0].detail, "field value: Int");

    let method = package
        .dependency_method_completions_at(source, nth_offset(source, ".ge", 1) + ".ge".len())
        .expect("dependency value-root method completions should survive parse errors");
    assert_eq!(
        method
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["get"]
    );
    assert!(method.iter().all(|item| item.kind == SymbolKind::Method));
    assert_eq!(method[0].detail, "fn get(self) -> Int");
}

#[test]
fn package_analysis_surfaces_dependency_local_method_result_member_completions_in_parse_error_source()
 {
    let temp =
        TempDir::new("ql-analysis-package-local-method-result-member-completion-parse-error");
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

pub struct Config {}

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
    let current = config.child()?
    return current.va + current.ge(
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field = package
        .dependency_member_field_completions_at(source, nth_offset(source, ".va", 1) + ".va".len())
        .expect("dependency local method-result field completions should survive parse errors");
    assert_eq!(
        field
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["value"]
    );
    assert!(field.iter().all(|item| item.kind == SymbolKind::Field));
    assert_eq!(field[0].detail, "field value: Int");

    let method = package
        .dependency_method_completions_at(source, nth_offset(source, ".ge", 1) + ".ge".len())
        .expect("dependency local method-result method completions should survive parse errors");
    assert_eq!(
        method
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["get"]
    );
    assert!(method.iter().all(|item| item.kind == SymbolKind::Method));
    assert_eq!(method[0].detail, "fn get(self) -> Int");
}
