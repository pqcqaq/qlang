use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies};
use ql_lsp::bridge::{
    declaration_for_dependency_methods, definition_for_dependency_methods,
    hover_for_dependency_methods, references_for_dependency_methods, span_to_range,
};
use tower_lsp::lsp_types::request::GotoDeclarationResponse;
use tower_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, Location, Position, Url};

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
fn dependency_extend_method_queries_work_on_question_unwrapped_local_method_receivers_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-unwrapped-local-extend-method-member-query-broken");
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

extend Child {
    pub fn get(self) -> Int
}

pub struct Config {}

impl Config {
    pub fn child(self) -> Option[Child]
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
    let first = current.get()
    let second = current.get()
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_offset = nth_offset(source, "get", 1);
    let second_offset = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, first_offset))
            .expect("question-unwrapped local extend-member hover should exist without semantic analysis");
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
    .expect("question-unwrapped local extend-member definition should exist without semantic analysis");
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
    .expect("question-unwrapped local extend-member declaration should exist without semantic analysis");
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
        offset_to_position(source, second_offset),
        true,
    )
    .expect("question-unwrapped local extend-member references should exist without semantic analysis");
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
    .expect("question-unwrapped local extend-member references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}
