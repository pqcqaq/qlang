use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_dependency_struct_fields, completion_for_dependency_variants,
    completion_for_package_analysis, definition_for_dependency_struct_fields,
    definition_for_package_analysis, span_to_range, type_definition_for_dependency_variants,
};
use tower_lsp::lsp_types::request::GotoTypeDefinitionResponse;
use tower_lsp::lsp_types::{
    CompletionItemKind, CompletionResponse, CompletionTextEdit, GotoDefinitionResponse, Location,
    Position, TextEdit,
};

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

fn assert_targets_dependency_enum(
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
    let start = artifact
        .find(snippet)
        .expect("enum signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + snippet.len()))
    );
}

#[test]
fn lsp_surfaces_dependency_variant_contracts_for_import_local_roots() {
    let temp = TempDir::new("ql-lsp-dependency-import-local-variant");
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry,
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
    let analysis = analyze_source(direct_source).expect("source should analyze");
    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        direct_source,
        &analysis,
        &package,
        offset_to_position(
            direct_source,
            nth_offset(direct_source, ".Re", 1) + ".Re".len(),
        ),
    ) else {
        panic!("direct import local root should expose dependency variant completion")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));

    let grouped_source = r#"
package demo.app

use demo.dep.{Command}

pub fn main() -> Int {
    let value = Command.Ret
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", grouped_source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let Some(CompletionResponse::Array(items)) = completion_for_dependency_variants(
        grouped_source,
        &package,
        offset_to_position(
            grouped_source,
            nth_offset(grouped_source, ".Ret", 1) + ".Ret".len(),
        ),
    ) else {
        panic!("grouped direct import local root should expose dependency variant completion")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));

    let grouped_query_source = r#"
package demo.app

use demo.dep.{Command}

pub fn main() -> Int {
    let value = Command.Retry
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", grouped_query_source);

    let definition = type_definition_for_dependency_variants(
        grouped_query_source,
        &package,
        offset_to_position(
            grouped_query_source,
            nth_offset(grouped_query_source, "Retry", 1),
        ),
    )
    .expect("grouped direct import local root should expose dependency variant type definition");
    assert_targets_dependency_enum(
        definition,
        &dep_qi,
        "pub enum Command {\n    Retry,\n    Stop,\n}",
    );
}

#[test]
fn lsp_surfaces_dependency_struct_field_contracts_for_import_local_roots() {
    let temp = TempDir::new("ql-lsp-dependency-import-local-struct-field");
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
    let analysis = analyze_source(direct_source).expect("source should analyze");
    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        direct_source,
        &analysis,
        &package,
        offset_to_position(
            direct_source,
            nth_offset(direct_source, "fl", 1) + "fl".len(),
        ),
    ) else {
        panic!("direct import local root should expose dependency struct field completion")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "flag");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field flag: Bool"));
    assert_eq!(
        items[0].text_edit,
        Some(CompletionTextEdit::Edit(TextEdit::new(
            span_to_range(
                direct_source,
                ql_span::Span::new(
                    nth_offset(direct_source, "fl", 1),
                    nth_offset(direct_source, "fl", 1) + "fl".len(),
                ),
            ),
            "flag".to_owned(),
        ))),
    );

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
    let Some(CompletionResponse::Array(items)) = completion_for_dependency_struct_fields(
        grouped_source,
        &package,
        offset_to_position(
            grouped_source,
            nth_offset(grouped_source, "fl", 1) + "fl".len(),
        ),
    ) else {
        panic!("grouped direct import local root should expose dependency struct field completion")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "flag");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));

    let definition = definition_for_dependency_struct_fields(
        grouped_source,
        &package,
        offset_to_position(grouped_source, nth_offset(grouped_source, "value", 1)),
    )
    .expect("grouped direct import local root should expose dependency struct field definition");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
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

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find("value")
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "value".len()))
    );
}

#[test]
fn lsp_surfaces_dependency_struct_field_contracts_for_deeper_import_local_roots() {
    let temp = TempDir::new("ql-lsp-dependency-deeper-import-local-struct-field");
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
    let next = Config.Scope.Config { value: current, fl: true }
    let Config.Scope.Config { value: reused, flag: enabled } = built
    if enabled {
        return next.value + reused
    }
    return reused
}
"#;
    temp.write("workspace/app/src/lib.ql", direct_source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(direct_source).expect("source should analyze");
    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        direct_source,
        &analysis,
        &package,
        offset_to_position(
            direct_source,
            nth_offset(direct_source, "fl", 1) + "fl".len(),
        ),
    ) else {
        panic!("deeper import local root should expose dependency struct field completion")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "flag");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field flag: Bool"));

    let definition = definition_for_package_analysis(
        &tower_lsp::lsp_types::Url::parse("file:///workspace/app/src/lib.ql")
            .expect("URI should parse"),
        direct_source,
        &analysis,
        &package,
        offset_to_position(direct_source, nth_offset(direct_source, "value", 1)),
    )
    .expect("deeper import local root should expose dependency struct field definition");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
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

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find("value")
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "value".len()))
    );

    let grouped_source = r#"
package demo.app

use demo.dep.Config

pub fn main(current: Int, built: Config) -> Int {
    let next = Config.Scope.Config { value: current, fl: true }
    let Config.Scope.Config { value: reused, flag: enabled } = built
    return missing
}
"#;
    temp.write("workspace/app/src/lib.ql", grouped_source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let Some(CompletionResponse::Array(items)) = completion_for_dependency_struct_fields(
        grouped_source,
        &package,
        offset_to_position(
            grouped_source,
            nth_offset(grouped_source, "fl", 1) + "fl".len(),
        ),
    ) else {
        panic!("deeper import local root should expose dependency struct field completion")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "flag");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
}
