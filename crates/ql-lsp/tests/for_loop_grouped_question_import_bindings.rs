use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_dependency_member_fields, completion_for_dependency_methods,
    completion_for_package_analysis, declaration_for_dependency_methods,
    declaration_for_dependency_struct_fields, declaration_for_package_analysis,
    definition_for_dependency_methods, definition_for_dependency_struct_fields,
    hover_for_dependency_methods, hover_for_dependency_struct_fields, hover_for_package_analysis,
    references_for_dependency_methods, references_for_dependency_struct_fields,
    references_for_package_analysis, span_to_range, type_definition_for_dependency_method_types,
    type_definition_for_dependency_struct_field_types, type_definition_for_package_analysis,
};
use tower_lsp::lsp_types::request::{GotoDeclarationResponse, GotoTypeDefinitionResponse};
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

fn assert_targets_dependency_type(
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
    let type_def = artifact
        .find(snippet)
        .expect("type signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(
            &artifact,
            ql_span::Span::new(type_def, type_def + snippet.len())
        )
    );
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
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
        panic!("grouped question function iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_if_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { maybe_items? } else { maybe_items? }) {
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped structured question static iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_if_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { maybe_items? } else { maybe_items? }) {
        let value = current.ge
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

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("grouped structured question static iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_if_question_function_iterables()
{
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
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
        panic!("grouped structured question function iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_if_question_function_iterables()
{
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let value = current.ge
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

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("grouped structured question function iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let value = current.ge
    }
    let broken: Int = "oops"
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
        panic!("grouped question function iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_match_question_function_iterables()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
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
        panic!("grouped match question function iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_match_question_function_iterables()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let value = current.ge
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

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("grouped match question function iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_match_question_static_iterables()
{
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped match question static iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_match_question_static_iterables()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let value = current.ge
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

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("grouped match question static iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let value = current.ge
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

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("grouped question static iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_question_static_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let value = current.ge
    }
    let broken: Int = "oops"
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
        panic!("grouped question static iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_if_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { maybe_items? } else { maybe_items? }) {
        let value = current.va
    }
    let broken: Int = "oops"
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped structured question static iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_if_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { maybe_items? } else { maybe_items? }) {
        let value = current.ge
    }
    let broken: Int = "oops"
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
        panic!("grouped structured question static iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_if_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-function-structured-field-broken-completion",
    );
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let value = current.va
    }
    let broken: Int = "oops"
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

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped structured question function iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_if_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-function-structured-method-broken-completion",
    );
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let value = current.ge
    }
    let broken: Int = "oops"
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
        panic!("grouped structured question function iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let value = current.ge
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

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("grouped question function iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_match_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-function-structured-field-broken-completion",
    );
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let value = current.va
    }
    let broken: Int = "oops"
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

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped match question function iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_match_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-function-structured-method-broken-completion",
    );
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let value = current.ge
    }
    let broken: Int = "oops"
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
        panic!("grouped match question function iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_match_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let value = current.va
    }
    let broken: Int = "oops"
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped match question static iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_methods_for_for_loop_grouped_match_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let value = current.ge
    }
    let broken: Int = "oops"
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
        panic!("grouped match question static iterable method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let value = current.va
    }
    let broken: Int = "oops"
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

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped question function iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped question static iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_fields_for_for_loop_grouped_question_static_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let value = current.va
    }
    let broken: Int = "oops"
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("grouped question static iterable field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-query");
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

pub fn read() -> Int {
    for current in kids()? {
        return current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped question function iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-query-broken");
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

pub fn read() -> Int {
    for current in kids()? {
        return current.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped question function iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-definition");
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

pub fn read() -> Int {
    for current in maybe_items? {
        return current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped question static iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_question_static_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-definition-broken");
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

pub fn read() -> Int {
    for current in maybe_items? {
        return current.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped question static iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-query");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let value = current.get()
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("grouped question static iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_question_static_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-query-broken");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let value = current.get()
    }
    let broken: Int = "oops"
    return 0
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
    .expect("grouped question static iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-definition");
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

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let value = current.get()
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("grouped question function iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-definition-broken");
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

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let value = current.get()
    }
    let broken: Int = "oops"
    return 0
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
    .expect("grouped question function iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_if_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-query");
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

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        return current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped structured question function iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_if_question_function_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-query-broken");
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

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        return current.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped structured question function iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_match_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-query");
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

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        return current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped match question static iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_field_definition_works_on_for_loop_grouped_match_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-query-broken");
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

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        return current.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".value", 1) + 1),
    )
    .expect("grouped match question static iterable field definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "value");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_if_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-query");
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

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let value = current.get()
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("grouped structured question function iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_if_question_function_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-query-broken");
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

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let value = current.get()
    }
    let broken: Int = "oops"
    return 0
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
    .expect("grouped structured question function iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_match_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-query");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let value = current.get()
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("grouped match question static iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_method_definition_works_on_for_loop_grouped_match_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-query-broken");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let value = current.get()
    }
    let broken: Int = "oops"
    return 0
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
    .expect("grouped match question static iterable method definition should exist");

    assert_targets_dependency_snippet(definition, &dep_qi, "get");
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-query");
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

pub fn read() -> Int {
    for current in kids()? {
        let first = current.value
        return current.value + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped question function iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped question function iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped question function iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect("grouped question function iterable field references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-query-broken");
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

pub fn read() -> Int {
    for current in kids()? {
        let first = current.value
        return current.value + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped question function iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped question function iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped question function iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect("grouped question function iterable field references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-query");
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

pub fn read() -> Int {
    for current in maybe_items? {
        let first = current.value
        return current.value + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped question static iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped question static iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped question static iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect("grouped question static iterable field references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_question_static_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-query-broken");
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

pub fn read() -> Int {
    for current in maybe_items? {
        let first = current.value
        return current.value + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped question static iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped question static iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped question static iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect("grouped question static iterable field references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-query");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let first = current.get()
        return current.get() + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
    )
    .expect("grouped question static iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped question static iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
        true,
    )
    .expect("grouped question static iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
        false,
    )
    .expect("grouped question static iterable method references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_question_static_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-query-broken");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        let first = current.get()
        return current.get() + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, first_method))
            .expect("grouped question static iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped question static iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, second_method),
        true,
    )
    .expect("grouped question static iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, first_method),
        false,
    )
    .expect("grouped question static iterable method references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-query");
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

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let first = current.get()
        return current.get() + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
    )
    .expect("grouped question function iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped question function iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
        true,
    )
    .expect("grouped question function iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
        false,
    )
    .expect(
        "grouped question function iterable method references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_question_function_iterables_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-query-broken");
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

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        let first = current.get()
        return current.get() + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, first_method))
            .expect("grouped question function iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped question function iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, second_method),
        true,
    )
    .expect("grouped question function iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, first_method),
        false,
    )
    .expect(
        "grouped question function iterable method references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_if_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-query");
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

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let first = current.value
        return current.value + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped structured question function iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped structured question function iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped structured question function iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect(
        "grouped structured question function iterable field references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_if_question_function_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-query-broken");
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

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let first = current.value
        return current.value + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped structured question function iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped structured question function iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped structured question function iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect(
        "grouped structured question function iterable field references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_match_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-query");
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

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let first = current.value
        return current.value + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped match question static iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped match question static iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped match question static iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect(
        "grouped match question static iterable field references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_grouped_match_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-query-broken");
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

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let first = current.value
        return current.value + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_field = nth_offset(source, "value", 1);
    let second_field = nth_offset(source, "value", 2);

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, first_field),
    )
    .expect("grouped match question static iterable field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let declaration = declaration_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, second_field),
    )
    .expect("grouped match question static iterable field declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
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

    let with_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, second_field),
        true,
    )
    .expect("grouped match question static iterable field references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(first_field, first_field + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        ql_span::Span::new(second_field, second_field + "value".len())
    )));

    let without_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, first_field),
        false,
    )
    .expect(
        "grouped match question static iterable field references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_if_question_function_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-query");
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

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let first = current.get()
        return current.get() + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
    )
    .expect("grouped structured question function iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped structured question function iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
        true,
    )
    .expect("grouped structured question function iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
        false,
    )
    .expect(
        "grouped structured question function iterable method references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_if_question_function_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-query-broken");
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

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        let first = current.get()
        return current.get() + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, first_method))
            .expect("grouped structured question function iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped structured question function iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, second_method),
        true,
    )
    .expect("grouped structured question function iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, first_method),
        false,
    )
    .expect(
        "grouped structured question function iterable method references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_match_question_static_iterables() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-query");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let first = current.get()
        return current.get() + first
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
    )
    .expect("grouped match question static iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped match question static iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, first_method),
        true,
    )
    .expect("grouped match question static iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, second_method),
        false,
    )
    .expect(
        "grouped match question static iterable method references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn dependency_method_queries_work_on_for_loop_grouped_match_question_static_iterables_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-query-broken");
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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        let first = current.get()
        return current.get() + first
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_method = nth_offset(source, "get", 1);
    let second_method = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, first_method))
            .expect("grouped match question static iterable method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, second_method),
    )
    .expect("grouped match question static iterable method declaration should exist");
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, first_method),
        true,
    )
    .expect("grouped match question static iterable method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let without_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, second_method),
        false,
    )
    .expect(
        "grouped match question static iterable method references should exist without declaration",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn type_definition_bridge_follows_grouped_question_function_iterable_field_types() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-field-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read() -> Int {
    for current in kids()? {
        return current.leaf.value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question function iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_function_iterable_field_types_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-field-type-definition-broken");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read() -> Int {
    for current in kids()? {
        return current.leaf.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_struct_field_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question function iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_static_iterable_field_types() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read() -> Int {
    for current in maybe_items? {
        return current.leaf.value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question static iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_static_iterable_field_types_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-field-type-definition-broken");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read() -> Int {
    for current in maybe_items? {
        return current.leaf.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_struct_field_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question static iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_static_iterable_method_return_types() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-static-method-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]

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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        return current.leaf().value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question static iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_static_iterable_method_return_types_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-method-type-definition-broken");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]

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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read() -> Int {
    for current in maybe_items? {
        return current.leaf().value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_method_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question static iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_function_iterable_method_return_types() {
    let temp = TempDir::new("ql-lsp-for-loop-grouped-question-function-method-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]

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

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        return current.leaf().value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question function iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_question_function_iterable_method_return_types_without_semantic_analysis()
 {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-method-type-definition-broken");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]

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

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    for current in kids()? {
        return current.leaf().value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_method_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped question function iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_if_question_function_iterable_field_types() {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-field-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        return current.leaf.value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped structured question function iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_if_question_function_iterable_field_types_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-function-structured-field-type-definition-broken",
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        return current.leaf.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_struct_field_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped structured question function iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_match_question_static_iterable_field_types() {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-field-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        return current.leaf.value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped match question static iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_match_question_static_iterable_field_types_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-static-structured-field-type-definition-broken",
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
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

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        return current.leaf.value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_struct_field_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped match question static iterable field type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_if_question_function_iterable_method_return_types() {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-function-structured-method-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]

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

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        return current.leaf().value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped structured question function iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_if_question_function_iterable_method_return_types_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-function-structured-method-type-definition-broken",
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]

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

use demo.dep.{maybe_children as kids}

pub fn read(flag: Bool) -> Int {
    for current in (if flag { kids()? } else { kids()? }) {
        return current.leaf().value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_method_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped structured question function iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_match_question_static_iterable_method_return_types() {
    let temp =
        TempDir::new("ql-lsp-for-loop-grouped-question-static-structured-method-type-definition");
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]

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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        return current.leaf().value
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
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped match question static iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}

#[test]
fn type_definition_bridge_follows_grouped_match_question_static_iterable_method_return_types_without_semantic_analysis()
 {
    let temp = TempDir::new(
        "ql-lsp-for-loop-grouped-question-static-structured-method-type-definition-broken",
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

pub struct Leaf {
    value: Int,
}

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]

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

use demo.dep.{MAYBE_ITEMS as maybe_items}

pub fn read(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_items?,
        false => maybe_items?,
    } {
        return current.leaf().value
    }
    let broken: Int = "oops"
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_method_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("grouped match question static iterable method type definition should exist");
    assert_targets_dependency_type(definition, &dep_qi, "pub struct Leaf {\n    value: Int,\n}");
}
