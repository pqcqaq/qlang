use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_dependency_member_fields, completion_for_dependency_methods,
    completion_for_package_analysis, definition_for_dependency_methods, span_to_range,
};
use tower_lsp::lsp_types::{
    CompletionItemKind, CompletionResponse, GotoDefinitionResponse, Location, Position,
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

fn assert_targets_dependency_snippet(
    definition: GotoDefinitionResponse,
    dep_qi: &Path,
    snippet: &str,
) {
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

    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .expect("snippet should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + snippet.len()))
    );
}

#[test]
fn package_bridge_completes_dependency_fields_for_if_question_function_iterable_receivers() {
    let temp = TempDir::new("ql-lsp-question-structured-iterable-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    for current in (if flag { maybe_children()? } else { maybe_children()? }) {
        let value = current.va
    }
    return 0
}
"#;
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("structured question iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_if_question_function_iterable_receivers_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-question-structured-iterable-field-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    for current in (if flag { maybe_children()? } else { maybe_children()? }) {
        let value = current.va
    }
    return "oops"
}
"#;
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("structured question iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_match_question_static_iterable_receivers_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-question-structured-iterable-method-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.MAYBE_ITEMS as items

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => items?,
        false => items?,
    } {
        let value = current.ge
    }
    return "oops"
}
"#;
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

pub static MAYBE_ITEMS: Option[[Child; 2]]

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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("structured question iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn dependency_method_definition_works_on_match_question_static_iterable_receivers_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-question-structured-iterable-method-query-broken");
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

use demo.dep.MAYBE_ITEMS as items

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => items?,
        false => items?,
    } {
        let value = current.get()
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("structured question iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}
