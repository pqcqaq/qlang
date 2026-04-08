use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    span_to_range, type_definition_for_dependency_method_types,
    type_definition_for_dependency_struct_field_types, type_definition_for_package_analysis,
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

#[derive(Clone, Copy)]
enum RootKind {
    Function,
    Static,
}

impl RootKind {
    fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Static => "static",
        }
    }

    fn use_decl(self) -> &'static str {
        match self {
            Self::Function => "use demo.dep.{maybe_load as load_cfg}",
            Self::Static => "use demo.dep.{MAYBE as maybe_cfg}",
        }
    }

    fn receiver_expr(self) -> &'static str {
        match self {
            Self::Function => "load_cfg()?",
            Self::Static => "maybe_cfg?",
        }
    }

    fn dep_member_decl(self) -> &'static str {
        match self {
            Self::Function => "pub fn maybe_load() -> Option[Child]",
            Self::Static => "pub static MAYBE: Option[Child]",
        }
    }
}

#[derive(Clone, Copy)]
enum MemberKind {
    Field,
    Method,
}

impl MemberKind {
    fn label(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Method => "method",
        }
    }

    fn member_token(self) -> &'static str {
        "leaf"
    }

    fn access_suffix(self) -> &'static str {
        match self {
            Self::Field => ".leaf.value",
            Self::Method => ".leaf().value",
        }
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

    fn wrap(self, expr: &str) -> String {
        match self {
            Self::If => format!("if flag {{ {expr} }} else {{ {expr} }}"),
            Self::Match => format!(
                "match flag {{\n        true => {expr},\n        false => {expr},\n    }}"
            ),
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

fn dependency_qi(root: RootKind, member: MemberKind) -> String {
    let method_block = match member {
        MemberKind::Field => String::new(),
        MemberKind::Method => "\nimpl Child {\n    pub fn leaf(self) -> Leaf\n}\n".to_string(),
    };
    format!(
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {{
    value: Int,
}}

pub struct Child {{
    leaf: Leaf,
}}

{dep_member}
{method_block}"#,
        dep_member = root.dep_member_decl(),
        method_block = method_block
    )
}

fn build_source(
    root: RootKind,
    member: MemberKind,
    structured: StructuredKind,
    broken: bool,
) -> String {
    let broken_line = if broken {
        "    let broken: Int = \"oops\"\n"
    } else {
        ""
    };
    format!(
        r#"
package demo.app

{use_decl}

pub fn read(flag: Bool) -> Int {{
{broken_line}    return ({wrapped}){suffix}
}}
"#,
        use_decl = root.use_decl(),
        wrapped = structured.wrap(root.receiver_expr()),
        suffix = member.access_suffix(),
        broken_line = broken_line
    )
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
        span_to_range(&artifact, ql_span::Span::new(type_def, type_def + snippet.len()))
    );
}

fn run_type_definition_case(
    root: RootKind,
    member: MemberKind,
    structured: StructuredKind,
    broken: bool,
) {
    let temp = TempDir::new(&format!(
        "ql-lsp-grouped-question-{}-{}-{}-member-type-definition{}",
        root.label(),
        structured.label(),
        member.label(),
        if broken { "-broken" } else { "" }
    ));
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
    let dep_qi = temp.write("workspace/dep/dep.qi", &dependency_qi(root, member));
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(root, member, structured, broken);
    temp.write("workspace/app/src/lib.ql", &source);
    let position = offset_to_position(&source, nth_offset(&source, member.member_token(), 1));

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let definition = match member {
            MemberKind::Field => type_definition_for_dependency_struct_field_types(
                &source, &package, position,
            ),
            MemberKind::Method => {
                type_definition_for_dependency_method_types(&source, &package, position)
            }
        }
        .expect("grouped structured question member type definition should exist");
        assert_targets_dependency_type(
            definition,
            &dep_qi,
            "pub struct Leaf {\n    value: Int,\n}",
        );
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis = analyze_source(&source).expect("source should analyze");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
        let definition =
            type_definition_for_package_analysis(&uri, &source, &analysis, &package, position)
                .expect("grouped structured question member type definition should exist");
        assert_targets_dependency_type(
            definition,
            &dep_qi,
            "pub struct Leaf {\n    value: Int,\n}",
        );
    }
}

#[test]
fn type_definition_bridge_follows_if_grouped_question_function_value_field_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Field,
        StructuredKind::If,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_if_grouped_question_function_value_field_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Field,
        StructuredKind::If,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_if_grouped_question_function_value_method_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Method,
        StructuredKind::If,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_if_grouped_question_function_value_method_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Method,
        StructuredKind::If,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_match_grouped_question_function_value_field_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Field,
        StructuredKind::Match,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_match_grouped_question_function_value_field_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Field,
        StructuredKind::Match,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_match_grouped_question_function_value_method_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Method,
        StructuredKind::Match,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_match_grouped_question_function_value_method_member_types() {
    run_type_definition_case(
        RootKind::Function,
        MemberKind::Method,
        StructuredKind::Match,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_if_grouped_question_static_value_field_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Field,
        StructuredKind::If,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_if_grouped_question_static_value_field_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Field,
        StructuredKind::If,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_if_grouped_question_static_value_method_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Method,
        StructuredKind::If,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_if_grouped_question_static_value_method_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Method,
        StructuredKind::If,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_match_grouped_question_static_value_field_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Field,
        StructuredKind::Match,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_match_grouped_question_static_value_field_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Field,
        StructuredKind::Match,
        true,
    );
}

#[test]
fn type_definition_bridge_follows_match_grouped_question_static_value_method_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Method,
        StructuredKind::Match,
        false,
    );
}

#[test]
fn type_definition_fallback_follows_match_grouped_question_static_value_method_member_types() {
    run_type_definition_case(
        RootKind::Static,
        MemberKind::Method,
        StructuredKind::Match,
        true,
    );
}
