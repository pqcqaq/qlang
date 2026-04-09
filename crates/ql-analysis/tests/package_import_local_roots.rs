use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, SymbolKind};

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
fn package_analysis_supports_dependency_variant_import_local_roots() {
    let temp = TempDir::new("ql-analysis-package-import-local-variant");
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.write(
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
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
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

    let direct_source = r#"
package demo.app

use demo.dep.Command

pub fn main() -> Int {
    return Command.Re()
}
"#;
    temp.write("workspace/app/src/lib.ql", direct_source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_variant_completions_at(
            direct_source,
            nth_offset(direct_source, ".Re", 1) + ".Re".len(),
        )
        .expect("direct import local root should expose dependency variant completion");
    assert_eq!(
        completions
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Retry", "Stop"]
    );
    assert!(completions
        .iter()
        .all(|item| item.kind == SymbolKind::Variant));

    let grouped_source = r#"
package demo.app

use demo.dep.{Command}

pub fn main() -> Int {
    let value = Command.Retry(1)
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", grouped_source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = package
        .dependency_variant_definition_in_source_at(
            grouped_source,
            nth_offset(grouped_source, "Retry", 1),
        )
        .expect("grouped direct import local root should expose dependency variant definition");
    assert_eq!(definition.kind, SymbolKind::Variant);
    assert_eq!(definition.name, "Retry");
    assert!(definition.path.ends_with("dep.qi"));

    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = artifact
        .get(definition.span.start..definition.span.end)
        .expect("definition span should slice artifact text");
    assert_eq!(snippet.trim(), "Retry");
}

#[test]
fn package_analysis_supports_dependency_struct_field_import_local_roots() {
    let temp = TempDir::new("ql-analysis-package-import-local-struct-field");
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.write(
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
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
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

    let direct_source = r#"
package demo.app

use demo.dep.Config

pub fn main(current: Int, built: Config) -> Int {
    let next = Config { value: current, fl: true }
    let Config { value: reused, flag: enabled } = built
    return next.value + reused
}
"#;
    temp.write("workspace/app/src/lib.ql", direct_source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let completions = package
        .dependency_struct_field_completions_at(
            direct_source,
            nth_offset(direct_source, "fl", 1) + "fl".len(),
        )
        .expect("direct import local root should expose dependency struct field completion");
    assert_eq!(
        completions
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["flag"]
    );
    assert!(completions
        .iter()
        .all(|item| item.kind == SymbolKind::Field));
    assert_eq!(completions[0].detail, "field flag: Bool");

    let grouped_source = r#"
package demo.app

use demo.dep.{Config}

pub fn main(current: Int, built: Config) -> Int {
    let next = Config { value: current, fl: true }
    let Config { value: reused, flag: enabled } = built
    return missing
}
"#;
    temp.write("workspace/app/src/lib.ql", grouped_source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = package
        .dependency_struct_field_definition_in_source_at(
            grouped_source,
            nth_offset(grouped_source, "value", 1),
        )
        .expect(
            "grouped direct import local root should expose dependency struct field definition",
        );
    assert_eq!(definition.kind, SymbolKind::Field);
    assert_eq!(definition.name, "value");
    assert!(definition.path.ends_with("dep.qi"));

    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = artifact
        .get(definition.span.start..definition.span.end)
        .expect("definition span should slice artifact text");
    assert_eq!(snippet.trim(), "value");
}
