use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_package_analysis, declaration_for_package_analysis,
    definition_for_package_analysis, hover_for_package_analysis, references_for_package_analysis,
    span_to_range, type_definition_for_package_analysis,
};
use tower_lsp::lsp_types::request::GotoDeclarationResponse;
use tower_lsp::lsp_types::request::GotoTypeDefinitionResponse;
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

#[derive(Clone, Copy)]
enum StructuredKind {
    If,
    Match,
}

impl StructuredKind {
    fn label(self) -> &'static str {
        match self {
            Self::If => "if",
            Self::Match => "match",
        }
    }

    fn receiver_expr(self) -> &'static str {
        match self {
            Self::If => "if flag { maybe_children()? } else { maybe_children()? }",
            Self::Match => {
                "match flag {\n        true => maybe_children()?,\n        false => maybe_children()?,\n    }"
            }
        }
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
    let start = artifact
        .find(snippet)
        .expect("type target should exist in dependency interface");
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + snippet.len()))
    );
}

fn write_dependency_files(temp: &TempDir) {
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
    value: Int,
    leaf: Leaf,
}

pub fn maybe_children() -> Option[[Child; 2]]

impl Child {
    pub fn get(self) -> Int
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
}

fn run_query_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-query",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let first = ({receiver})[0].get()
    let second = ({receiver})[1].get()
    return first + second
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let first_offset = nth_offset(&source, "get", 1);
    let second_offset = nth_offset(&source, "get", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect("structured question indexed method hover should exist through package bridge");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect("structured question indexed method definition should exist through package bridge");
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_dependency_name(
        &definition_location,
        &dep_qi,
        "pub fn get(self) -> Int",
        "get",
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        true,
    )
    .expect("structured question indexed method references should exist through package bridge");
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
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        false,
    )
    .expect(
        "structured question indexed method references without declaration should exist through package bridge",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

fn run_completion_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-completion",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].ge
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(&source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("structured question indexed method completion should exist through package bridge");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

fn run_bracket_target_method_completion_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-method-completion",
        structured.label()
    ));
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

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Leaf {{
    let value = ({receiver})[0].lea
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(&source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, ".lea", 1) + ".lea".len()),
    ) else {
        panic!(
            "structured question indexed bracket target method completion should exist through package bridge"
        );
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "leaf");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[0].detail.as_deref(), Some("fn leaf(self) -> Leaf"));
}

fn run_value_root_completion_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-value-root-completion",
        structured.label()
    ));
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

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].leaf().va
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(&source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, ".va", 1) + ".va".len()),
    ) else {
        panic!(
            "structured question indexed method-result value root completion should exist through package bridge"
        );
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

fn run_bracket_target_value_root_completion_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-value-root-completion",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].va
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(&source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, ".va", 1) + ".va".len()),
    ) else {
        panic!(
            "structured question indexed bracket target value root completion should exist through package bridge"
        );
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

fn run_bracket_target_field_completion_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-field-completion",
        structured.label()
    ));
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

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Leaf {{
    let value = ({receiver})[0].lea
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(&source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, ".lea", 1) + ".lea".len()),
    ) else {
        panic!(
            "structured question indexed bracket target field completion should exist through package bridge"
        );
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "leaf");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field leaf: Leaf"));
}

fn run_method_result_member_completion_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-result-member-completion",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].leaf().va
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(&source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, ".va", 1) + ".va".len()),
    ) else {
        panic!(
            "structured question indexed method-result member completion should exist through package bridge"
        );
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

fn run_method_result_member_query_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-result-member-query",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");

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

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let first = ({receiver})[0].leaf().value
    let second = ({receiver})[1].leaf().value
    return first + second
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let first_offset = nth_offset(&source, "value", 1);
    let second_offset = nth_offset(&source, "value", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect("structured question indexed method-result member hover should exist through package bridge");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect(
        "structured question indexed method-result member definition should exist through package bridge",
    );
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_dependency_name(&definition_location, &dep_qi, "value: Int", "value");

    let declaration = declaration_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect(
        "structured question indexed method-result member declaration should exist through package bridge",
    );
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_location_targets_dependency_name(&declaration_location, &dep_qi, "value: Int", "value");

    let with_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        true,
    )
    .expect(
        "structured question indexed method-result member references should exist through package bridge",
    );
    assert_eq!(with_declaration.len(), 3);
    assert_location_targets_dependency_name(&with_declaration[0], &dep_qi, "value: Int", "value");
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        false,
    )
    .expect(
        "structured question indexed method-result member references without declaration should exist through package bridge",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

fn run_method_result_member_type_definition_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-result-member-type-definition",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");

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

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].leaf().value
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let definition = type_definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, "leaf", 1)),
    )
    .expect(
        "structured question indexed method-result member type definition should exist through package bridge",
    );
    assert_targets_dependency_type(
        definition,
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );
}

fn run_type_definition_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-type-definition",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].leaf().value
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let definition = type_definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, "leaf", 1)),
    )
    .expect("structured question indexed method type definition should exist through package bridge");
    assert_targets_dependency_type(
        definition,
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );
}

fn run_bracket_target_value_root_query_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-value-root-query",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let first = ({receiver})[0].value
    let second = ({receiver})[1].value
    return first + second
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");
    let first_offset = nth_offset(&source, "maybe_children", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect(
        "structured question indexed bracket target value root hover should exist through package bridge",
    );
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect(
        "structured question indexed bracket target value root definition should exist through package bridge",
    );
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_targets_dependency_type(
        GotoTypeDefinitionResponse::Scalar(definition_location),
        &dep_qi,
        "pub struct Child {\n    value: Int,\n    leaf: Leaf,\n}",
    );

    let declaration = declaration_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect(
        "structured question indexed bracket target value root declaration should exist through package bridge",
    );
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_targets_dependency_type(
        GotoTypeDefinitionResponse::Scalar(declaration_location),
        &dep_qi,
        "pub struct Child {\n    value: Int,\n    leaf: Leaf,\n}",
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        true,
    )
    .expect(
        "structured question indexed bracket target value root references should exist through package bridge",
    );
    assert_eq!(with_declaration.len(), 6);
    assert_targets_dependency_type(
        GotoTypeDefinitionResponse::Scalar(with_declaration[0].clone()),
        &dep_qi,
        "pub struct Child {\n    value: Int,\n    leaf: Leaf,\n}",
    );
    assert_eq!(
        &with_declaration[1..],
        &[
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 1),
                        nth_offset(&source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 2),
                        nth_offset(&source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 3),
                        nth_offset(&source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 4),
                        nth_offset(&source, "maybe_children", 4) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 5),
                        nth_offset(&source, "maybe_children", 5) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        false,
    )
    .expect(
        "structured question indexed bracket target value root references without declaration should exist through package bridge",
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 2),
                        nth_offset(&source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 3),
                        nth_offset(&source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 4),
                        nth_offset(&source, "maybe_children", 4) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "maybe_children", 5),
                        nth_offset(&source, "maybe_children", 5) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

fn run_bracket_target_value_root_type_definition_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-value-root-type-definition",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].value
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let definition = type_definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, "maybe_children", 2)),
    )
    .expect(
        "structured question indexed bracket target value root type definition should exist through package bridge",
    );
    assert_targets_dependency_type(
        definition,
        &dep_qi,
        "pub struct Child {\n    value: Int,\n    leaf: Leaf,\n}",
    );
}

fn run_bracket_target_field_type_definition_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-field-type-definition",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let value = ({receiver})[0].leaf.value
    return value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let definition = type_definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, "leaf", 1)),
    )
    .expect(
        "structured question indexed bracket target field type definition should exist through package bridge",
    );
    assert_targets_dependency_type(
        definition,
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );
}

fn run_bracket_target_field_query_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-field-query",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let first = ({receiver})[0].leaf.value
    let second = ({receiver})[1].leaf.value
    return first + second
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let first_offset = nth_offset(&source, "leaf", 1);
    let second_offset = nth_offset(&source, "leaf", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect("structured question indexed bracket target field hover should exist through package bridge");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `leaf`"));
    assert!(markup.value.contains("field leaf: Leaf"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect(
        "structured question indexed bracket target field definition should exist through package bridge",
    );
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_dependency_name(
        &definition_location,
        &dep_qi,
        "leaf: Leaf",
        "leaf",
    );

    let declaration = declaration_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect(
        "structured question indexed bracket target field declaration should exist through package bridge",
    );
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_location_targets_dependency_name(
        &declaration_location,
        &dep_qi,
        "leaf: Leaf",
        "leaf",
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        true,
    )
    .expect(
        "structured question indexed bracket target field references should exist through package bridge",
    );
    assert_eq!(with_declaration.len(), 3);
    assert_location_targets_dependency_name(
        &with_declaration[0],
        &dep_qi,
        "leaf: Leaf",
        "leaf",
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        false,
    )
    .expect(
        "structured question indexed bracket target field references without declaration should exist through package bridge",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

fn run_bracket_target_method_query_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-bracket-target-method-query",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let first = ({receiver})[0].leaf().value
    let second = ({receiver})[1].leaf().value
    return first + second
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let first_offset = nth_offset(&source, "leaf", 1);
    let second_offset = nth_offset(&source, "leaf", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
    )
    .expect("structured question indexed bracket target method hover should exist through package bridge");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `leaf`"));
    assert!(markup.value.contains("fn leaf(self) -> Leaf"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect(
        "structured question indexed bracket target method definition should exist through package bridge",
    );
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_dependency_name(
        &definition_location,
        &dep_qi,
        "pub fn leaf(self) -> Leaf",
        "leaf",
    );

    let declaration = declaration_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, second_offset),
    )
    .expect(
        "structured question indexed bracket target method declaration should exist through package bridge",
    );
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_location_targets_dependency_name(
        &declaration_location,
        &dep_qi,
        "pub fn leaf(self) -> Leaf",
        "leaf",
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        true,
    )
    .expect(
        "structured question indexed bracket target method references should exist through package bridge",
    );
    assert_eq!(with_declaration.len(), 3);
    assert_location_targets_dependency_name(
        &with_declaration[0],
        &dep_qi,
        "pub fn leaf(self) -> Leaf",
        "leaf",
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, first_offset),
        false,
    )
    .expect(
        "structured question indexed bracket target method references without declaration should exist through package bridge",
    );
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

fn run_value_root_query_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-value-root-query",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let current = ({receiver})[0].leaf()
    return current.value + current.value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");
    let current_usage = nth_offset(&source, "current", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
    )
    .expect("structured question indexed method-result value root hover should exist through package bridge");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Leaf`"));
    assert!(markup.value.contains("struct Leaf"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
    )
    .expect("structured question indexed method-result value root definition should exist through package bridge");
    let GotoDefinitionResponse::Scalar(definition_location) = definition else {
        panic!("definition should be one location")
    };
    assert_targets_dependency_type(
        GotoTypeDefinitionResponse::Scalar(definition_location),
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );

    let declaration = declaration_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
    )
    .expect("structured question indexed method-result value root declaration should exist through package bridge");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_targets_dependency_type(
        GotoTypeDefinitionResponse::Scalar(declaration_location),
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
        true,
    )
    .expect(
        "structured question indexed method-result value root references should exist through package bridge",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_targets_dependency_type(
        GotoTypeDefinitionResponse::Scalar(with_declaration[0].clone()),
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );
    assert_eq!(
        &with_declaration[1..],
        &[
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "current", 1),
                        nth_offset(&source, "current", 1) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "current", 2),
                        nth_offset(&source, "current", 2) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "current", 3),
                        nth_offset(&source, "current", 3) + "current".len(),
                    ),
                ),
            ),
        ]
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
        false,
    )
    .expect(
        "structured question indexed method-result value root references without declaration should exist through package bridge",
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "current", 2),
                        nth_offset(&source, "current", 2) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    &source,
                    ql_span::Span::new(
                        nth_offset(&source, "current", 3),
                        nth_offset(&source, "current", 3) + "current".len(),
                    ),
                ),
            ),
        ]
    );
}

fn run_value_root_type_definition_case(structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-package-bridge-structured-question-indexed-{}-method-value-root-type-definition",
        structured.label()
    ));
    let app_root = temp.path().join("workspace").join("app");
    let dep_qi = temp.path().join("workspace").join("dep").join("dep.qi");
    write_dependency_files(&temp);

    let source = format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let current = ({receiver})[0].leaf()
    return current.value
}}
"#,
        receiver = structured.receiver_expr(),
    );
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("app path should convert to file URL");
    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");

    let definition = type_definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, nth_offset(&source, "current", 2)),
    )
    .expect(
        "structured question indexed method-result value root type definition should exist through package bridge",
    );
    assert_targets_dependency_type(
        definition,
        &dep_qi,
        "pub struct Leaf {\n    value: Int,\n}",
    );
}

#[test]
fn package_bridge_surfaces_if_direct_structured_question_indexed_method_queries() {
    run_query_case(StructuredKind::If);
}

#[test]
fn package_bridge_surfaces_match_direct_structured_question_indexed_method_queries() {
    run_query_case(StructuredKind::Match);
}

#[test]
fn package_bridge_completes_if_direct_structured_question_indexed_methods() {
    run_completion_case(StructuredKind::If);
}

#[test]
fn package_bridge_completes_match_direct_structured_question_indexed_methods() {
    run_completion_case(StructuredKind::Match);
}

#[test]
fn package_bridge_completes_if_direct_structured_question_indexed_bracket_target_methods() {
    run_bracket_target_method_completion_case(StructuredKind::If);
}

#[test]
fn package_bridge_completes_match_direct_structured_question_indexed_bracket_target_methods() {
    run_bracket_target_method_completion_case(StructuredKind::Match);
}

#[test]
fn package_bridge_follows_if_direct_structured_question_indexed_method_type_definitions() {
    run_type_definition_case(StructuredKind::If);
}

#[test]
fn package_bridge_follows_match_direct_structured_question_indexed_method_type_definitions() {
    run_type_definition_case(StructuredKind::Match);
}

#[test]
fn package_bridge_completes_if_direct_structured_question_indexed_bracket_target_value_roots() {
    run_bracket_target_value_root_completion_case(StructuredKind::If);
}

#[test]
fn package_bridge_surfaces_if_direct_structured_question_indexed_bracket_target_value_root_queries()
{
    run_bracket_target_value_root_query_case(StructuredKind::If);
}

#[test]
fn package_bridge_completes_match_direct_structured_question_indexed_bracket_target_value_roots() {
    run_bracket_target_value_root_completion_case(StructuredKind::Match);
}

#[test]
fn package_bridge_surfaces_match_direct_structured_question_indexed_bracket_target_value_root_queries(
) {
    run_bracket_target_value_root_query_case(StructuredKind::Match);
}

#[test]
fn package_bridge_follows_if_direct_structured_question_indexed_bracket_target_value_root_type_definitions(
) {
    run_bracket_target_value_root_type_definition_case(StructuredKind::If);
}

#[test]
fn package_bridge_follows_match_direct_structured_question_indexed_bracket_target_value_root_type_definitions(
) {
    run_bracket_target_value_root_type_definition_case(StructuredKind::Match);
}

#[test]
fn package_bridge_completes_if_direct_structured_question_indexed_bracket_target_fields() {
    run_bracket_target_field_completion_case(StructuredKind::If);
}

#[test]
fn package_bridge_completes_match_direct_structured_question_indexed_bracket_target_fields() {
    run_bracket_target_field_completion_case(StructuredKind::Match);
}

#[test]
fn package_bridge_follows_if_direct_structured_question_indexed_bracket_target_field_type_definitions(
) {
    run_bracket_target_field_type_definition_case(StructuredKind::If);
}

#[test]
fn package_bridge_follows_match_direct_structured_question_indexed_bracket_target_field_type_definitions(
) {
    run_bracket_target_field_type_definition_case(StructuredKind::Match);
}

#[test]
fn package_bridge_surfaces_if_direct_structured_question_indexed_bracket_target_field_queries() {
    run_bracket_target_field_query_case(StructuredKind::If);
}

#[test]
fn package_bridge_surfaces_match_direct_structured_question_indexed_bracket_target_field_queries() {
    run_bracket_target_field_query_case(StructuredKind::Match);
}

#[test]
fn package_bridge_surfaces_if_direct_structured_question_indexed_bracket_target_method_queries() {
    run_bracket_target_method_query_case(StructuredKind::If);
}

#[test]
fn package_bridge_surfaces_match_direct_structured_question_indexed_bracket_target_method_queries() {
    run_bracket_target_method_query_case(StructuredKind::Match);
}

#[test]
fn package_bridge_surfaces_if_direct_structured_question_indexed_method_value_root_queries() {
    run_value_root_query_case(StructuredKind::If);
}

#[test]
fn package_bridge_completes_if_direct_structured_question_indexed_method_value_roots() {
    run_value_root_completion_case(StructuredKind::If);
}

#[test]
fn package_bridge_surfaces_match_direct_structured_question_indexed_method_value_root_queries() {
    run_value_root_query_case(StructuredKind::Match);
}

#[test]
fn package_bridge_completes_match_direct_structured_question_indexed_method_value_roots() {
    run_value_root_completion_case(StructuredKind::Match);
}

#[test]
fn package_bridge_completes_if_direct_structured_question_indexed_method_result_members() {
    run_method_result_member_completion_case(StructuredKind::If);
}

#[test]
fn package_bridge_completes_match_direct_structured_question_indexed_method_result_members() {
    run_method_result_member_completion_case(StructuredKind::Match);
}

#[test]
fn package_bridge_surfaces_if_direct_structured_question_indexed_method_result_member_queries() {
    run_method_result_member_query_case(StructuredKind::If);
}

#[test]
fn package_bridge_surfaces_match_direct_structured_question_indexed_method_result_member_queries() {
    run_method_result_member_query_case(StructuredKind::Match);
}

#[test]
fn package_bridge_follows_if_direct_structured_question_indexed_method_result_member_type_definitions(
) {
    run_method_result_member_type_definition_case(StructuredKind::If);
}

#[test]
fn package_bridge_follows_match_direct_structured_question_indexed_method_result_member_type_definitions(
) {
    run_method_result_member_type_definition_case(StructuredKind::Match);
}

#[test]
fn package_bridge_follows_if_direct_structured_question_indexed_method_value_root_type_definitions(
) {
    run_value_root_type_definition_case(StructuredKind::If);
}

#[test]
fn package_bridge_follows_match_direct_structured_question_indexed_method_value_root_type_definitions(
) {
    run_value_root_type_definition_case(StructuredKind::Match);
}
