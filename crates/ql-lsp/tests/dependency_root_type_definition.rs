use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    span_to_range, type_definition_for_dependency_values, type_definition_for_package_analysis,
};
use tower_lsp::lsp_types::request::GotoTypeDefinitionResponse;
use tower_lsp::lsp_types::{Location, Position, Url};

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

fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Position::new(line, prefix[line_start..].chars().count() as u32)
}

fn assert_targets_dependency_struct(
    definition: GotoTypeDefinitionResponse,
    dep_qi: &Path,
    snippet: &str,
) {
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("type definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let struct_def = artifact
        .find(snippet)
        .expect("struct signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(
            &artifact,
            ql_span::Span::new(struct_def, struct_def + snippet.len())
        )
    );
}

#[test]
fn type_definition_bridge_follows_dependency_function_call_roots() {
    let temp = TempDir::new("ql-lsp-dependency-function-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn load() -> Config
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

pub fn main() -> Int {
    return load().value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "load", 2)),
    )
    .expect("dependency function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_static_roots() {
    let temp = TempDir::new("ql-lsp-dependency-static-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.MAYBE as cfg

pub fn main() -> Int {
    return cfg?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "cfg", 2)),
    )
    .expect("dependency static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_function_roots() {
    let temp = TempDir::new("ql-lsp-dependency-question-function-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.maybe_load

pub fn main() -> Int {
    return maybe_load()?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_load", 2)),
    )
    .expect("dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_dependency_const_roots_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-dependency-const-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub const DEFAULT: Config
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

use demo.dep.DEFAULT as cfg

pub fn main() -> Int {
    let next = cfg.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "cfg", 2)),
    )
    .expect("dependency const root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_static_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-dependency-question-static-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.MAYBE as child

pub fn main() -> Int {
    let next = child?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "child", 2)),
    )
    .expect("dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_function_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-dependency-question-function-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.maybe_load

pub fn main() -> Int {
    let next = maybe_load()?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_load", 2)),
    )
    .expect("dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-dependency-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn children() -> [Child; 2]
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

use demo.dep.children

pub fn main() -> Int {
    for current in children() {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "children", 2)),
    )
    .expect("dependency iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_dependency_const_iterable_roots() {
    let temp = TempDir::new("ql-lsp-dependency-const-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub const ITEMS: [Child; 2]
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

use demo.dep.ITEMS

pub fn main() -> Int {
    for current in ITEMS {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "ITEMS", 2)),
    )
    .expect("dependency iterable const root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_dependency_const_iterable_roots_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-dependency-const-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub const ITEMS: [Child; 2]
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

use demo.dep.ITEMS

pub fn main() -> Int {
    for current in ITEMS {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "ITEMS", 2)),
    )
    .expect("dependency iterable const root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn children() -> [Child; 2]
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

use demo.dep.{children as kids}

pub fn main() -> Int {
    for current in kids() {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped dependency iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_dependency_const_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-const-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub const ITEMS: [Child; 2]
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

use demo.dep.{ITEMS as items}

pub fn main() -> Int {
    for current in items {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "items", 2)),
    )
    .expect("grouped dependency iterable const root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_dependency_const_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-const-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub const ITEMS: [Child; 2]
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

use demo.dep.{ITEMS as items}

pub fn main() -> Int {
    for current in items {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "items", 2)),
    )
    .expect("grouped dependency iterable const root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-question-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

pub fn main() -> Int {
    for current in maybe_children()? {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_children", 2)),
    )
    .expect("dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_structured_question_wrapped_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-structured-question-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

pub fn main(flag: Bool) -> Int {
    for current in (if flag { maybe_children()? } else { maybe_children()? }) {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_children", 2)),
    )
    .expect("structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_match_structured_question_wrapped_dependency_function_iterable_roots(
) {
    let temp =
        TempDir::new("ql-lsp-match-structured-question-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

pub fn main(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    } {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_children", 2)),
    )
    .expect("match structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-function-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

pub fn main() -> Int {
    for current in maybe_children()? {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_children", 2)),
    )
    .expect("dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-structured-question-function-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

pub fn main(flag: Bool) -> Int {
    for current in (if flag { maybe_children()? } else { maybe_children()? }) {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_children", 2)),
    )
    .expect("structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_match_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new(
        "ql-lsp-match-structured-question-function-iterable-root-type-definition-broken",
    );
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

pub fn main(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    } {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_children", 2)),
    )
    .expect("match structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_static_iterable_roots() {
    let temp = TempDir::new("ql-lsp-question-static-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]
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

use demo.dep.MAYBE_ITEMS as items

pub fn main() -> Int {
    for current in items? {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "items", 2)),
    )
    .expect("dependency question iterable static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_question_wrapped_dependency_static_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-static-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]
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

use demo.dep.MAYBE_ITEMS as items

pub fn main() -> Int {
    for current in items? {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "items", 2)),
    )
    .expect("dependency question iterable static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_function_iterable_roots(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

use demo.dep.{maybe_children as kids}

pub fn main() -> Int {
    for current in kids()? {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_structured_question_wrapped_dependency_function_iterable_roots(
) {
    let temp = TempDir::new(
        "ql-lsp-grouped-structured-question-function-iterable-root-type-definition",
    );
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

use demo.dep.{maybe_children as kids}

pub fn main(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_match_structured_question_wrapped_dependency_function_iterable_roots(
) {
    let temp =
        TempDir::new("ql-lsp-grouped-match-question-function-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

use demo.dep.{maybe_children as kids}

pub fn main(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped match structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_static_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-static-iterable-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn main() -> Int {
    for current in maybe_items? {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_items", 2)),
    )
    .expect("grouped dependency question iterable static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-function-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

use demo.dep.{maybe_children as kids}

pub fn main() -> Int {
    for current in kids()? {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new(
        "ql-lsp-grouped-structured-question-function-iterable-root-type-definition-broken",
    );
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

use demo.dep.{maybe_children as kids}

pub fn main(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_match_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new(
        "ql-lsp-grouped-match-question-function-iterable-root-type-definition-broken",
    );
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
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

use demo.dep.{maybe_children as kids}

pub fn main(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "kids", 2)),
    )
    .expect("grouped match structured dependency question iterable function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_static_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-static-iterable-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn main() -> Int {
    for current in maybe_items? {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_items", 2)),
    )
    .expect("grouped dependency question iterable static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_function_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-function-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main() -> Int {
    return load_cfg()?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "load_cfg", 2)),
    )
    .expect("grouped dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_structured_question_wrapped_dependency_function_roots(
) {
    let temp = TempDir::new("ql-lsp-grouped-structured-question-function-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main(flag: Bool) -> Int {
    return (if flag { load_cfg()? } else { load_cfg()? }).value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "load_cfg", 2)),
    )
    .expect("grouped structured dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_match_structured_question_wrapped_dependency_function_roots(
) {
    let temp = TempDir::new("ql-lsp-grouped-match-question-function-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main(flag: Bool) -> Int {
    return (match flag {
        true => load_cfg()?,
        false => load_cfg()?,
    }).value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "load_cfg", 2)),
    )
    .expect("grouped match structured dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_function_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-function-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main() -> Int {
    let next = load_cfg()?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "load_cfg", 2)),
    )
    .expect("grouped dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_structured_question_wrapped_dependency_function_roots_without_semantic_analysis(
) {
    let temp =
        TempDir::new("ql-lsp-grouped-structured-question-function-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main(flag: Bool) -> Int {
    let next = (if flag { load_cfg()? } else { load_cfg()? }).value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "load_cfg", 2)),
    )
    .expect("grouped structured dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_match_structured_question_wrapped_dependency_function_roots_without_semantic_analysis(
) {
    let temp =
        TempDir::new("ql-lsp-grouped-match-question-function-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main(flag: Bool) -> Int {
    let next = (match flag {
        true => load_cfg()?,
        false => load_cfg()?,
    }).value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "load_cfg", 2)),
    )
    .expect("grouped match structured dependency question function root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_static_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-static-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main() -> Int {
    return maybe_cfg?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_cfg", 2)),
    )
    .expect("grouped dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_structured_question_wrapped_dependency_static_roots(
) {
    let temp = TempDir::new("ql-lsp-grouped-structured-question-static-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main(flag: Bool) -> Int {
    return (if flag { maybe_cfg? } else { maybe_cfg? }).value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_cfg", 2)),
    )
    .expect("grouped structured dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_match_structured_question_wrapped_dependency_static_roots(
) {
    let temp = TempDir::new("ql-lsp-grouped-match-question-static-root-type-definition");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main(flag: Bool) -> Int {
    return (match flag {
        true => maybe_cfg?,
        false => maybe_cfg?,
    }).value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_cfg", 2)),
    )
    .expect("grouped match structured dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_question_wrapped_dependency_static_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-static-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main() -> Int {
    let next = maybe_cfg?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_cfg", 2)),
    )
    .expect("grouped dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_structured_question_wrapped_dependency_static_roots_without_semantic_analysis(
) {
    let temp =
        TempDir::new("ql-lsp-grouped-structured-question-static-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main(flag: Bool) -> Int {
    let next = (if flag { maybe_cfg? } else { maybe_cfg? }).value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_cfg", 2)),
    )
    .expect("grouped structured dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}

#[test]
fn type_definition_bridge_follows_grouped_import_match_structured_question_wrapped_dependency_static_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-match-question-static-root-type-definition-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main(flag: Bool) -> Int {
    let next = (match flag {
        true => maybe_cfg?,
        false => maybe_cfg?,
    }).value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "maybe_cfg", 2)),
    )
    .expect("grouped match structured dependency question static root type definition should exist");
    assert_targets_dependency_struct(
        definition,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
}
