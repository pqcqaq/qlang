use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_package_analysis, declaration_for_dependency_methods,
    declaration_for_package_analysis, definition_for_dependency_methods,
    definition_for_package_analysis, hover_for_dependency_methods,
    hover_for_package_analysis, references_for_dependency_methods,
    references_for_package_analysis, span_to_range,
};
use tower_lsp::lsp_types::request::GotoDeclarationResponse;
use tower_lsp::lsp_types::{
    CompletionItemKind, CompletionResponse, GotoDefinitionResponse, HoverContents, Location,
    Position, Url,
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

fn dependency_name_range(dep_qi: &Path, anchor: &str, name: &str) -> tower_lsp::lsp_types::Range {
    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let anchor_start = artifact
        .find(anchor)
        .expect("anchor should exist in dependency artifact");
    let name_start = anchor_start
        + anchor
            .find(name)
            .expect("member name should exist inside dependency anchor");
    span_to_range(
        &artifact,
        ql_span::Span::new(name_start, name_start + name.len()),
    )
}

fn assert_location_targets_dependency_name(
    location: &Location,
    dep_qi: &Path,
    anchor: &str,
    name: &str,
) {
    assert_eq!(
        location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert_eq!(location.range, dependency_name_range(dep_qi, anchor, name));
}

#[test]
fn dependency_field_completion_works_on_direct_match_question_indexed_receivers() {
    let temp = TempDir::new("ql-lsp-direct-match-question-indexed-field-completion");
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
    return (match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    })[0].va
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");
    let position = offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len());

    let Some(CompletionResponse::Array(items)) =
        completion_for_package_analysis(source, &analysis, &package, position)
    else {
        panic!("direct match question indexed field completion should exist");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn dependency_method_completion_works_on_direct_match_question_indexed_receivers() {
    let temp = TempDir::new("ql-lsp-direct-match-question-indexed-method-completion");
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

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let value = (match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    })[0].ge
    return value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");
    let position = offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len());

    let Some(CompletionResponse::Array(items)) =
        completion_for_package_analysis(source, &analysis, &package, position)
    else {
        panic!("direct match question indexed method completion should exist");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn dependency_method_queries_work_on_direct_match_question_indexed_receivers_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-direct-match-question-indexed-method-queries-broken");
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

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    })[0].get()
    let second = (match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    })[1].get()
    return "oops"
}
"#;
    let app_file = temp.write("workspace/app/src/lib.ql", source);
    let uri = Url::from_file_path(&app_file).expect("test file path should convert to URL");

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let first_offset = nth_offset(source, "get", 1);
    let second_offset = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, first_offset))
            .expect(
                "direct match question indexed method hover should exist without semantic analysis",
            );
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, first_offset),
    )
    .expect(
        "direct match question indexed method definition should exist without semantic analysis",
    );
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_dependency_name(
        &definition_location,
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, second_offset),
    )
    .expect(
        "direct match question indexed method declaration should exist without semantic analysis",
    );
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_location_targets_dependency_name(
        &declaration_location,
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );

    let with_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, first_offset),
        true,
    )
    .expect(
        "direct match question indexed method references should exist without semantic analysis",
    );
    assert_eq!(with_declaration.len(), 3);
    assert_location_targets_dependency_name(
        &with_declaration[0],
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );

    let without_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, first_offset),
        false,
    )
    .expect(
        "direct match question indexed method references should exist without declaration in fallback",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_direct_match_question_indexed_receivers() {
    let temp = TempDir::new("ql-lsp-direct-match-question-indexed-method-queries");
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

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    })[0].get()
    let second = (match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    })[1].get()
    return first + second
}
"#;
    let app_file = temp.write("workspace/app/src/lib.ql", source);
    let uri = Url::from_file_path(&app_file).expect("test file path should convert to URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let first_offset = nth_offset(source, "get", 1);
    let second_offset = nth_offset(source, "get", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_offset),
    )
    .expect("direct match question indexed method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_offset),
    )
    .expect("direct match question indexed method definition should exist");
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_dependency_name(
        &definition_location,
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_offset),
    )
    .expect("direct match question indexed method declaration should exist");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_location_targets_dependency_name(
        &declaration_location,
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_offset),
        true,
    )
    .expect("direct match question indexed method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_location_targets_dependency_name(
        &with_declaration[0],
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_offset),
        false,
    )
    .expect("direct match question indexed method references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}
