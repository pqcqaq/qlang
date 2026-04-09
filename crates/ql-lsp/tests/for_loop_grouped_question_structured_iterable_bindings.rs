use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies};
use ql_lsp::bridge::{
    definition_for_dependency_methods, definition_for_dependency_struct_fields, span_to_range,
};
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position};

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

    fn token(self) -> &'static str {
        match self {
            Self::Field => "value",
            Self::Method => "get",
        }
    }

    fn access_expr(self) -> &'static str {
        match self {
            Self::Field => "current.value",
            Self::Method => "current.get()",
        }
    }

    fn dep_impl(self) -> &'static str {
        match self {
            Self::Field => "",
            Self::Method => {
                r#"

impl Child {
    pub fn get(self) -> Int
}
"#
            }
        }
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
            Self::Function => "use demo.dep.{maybe_children as kids}",
            Self::Static => "use demo.dep.{MAYBE_ITEMS as maybe_items}",
        }
    }

    fn receiver_expr(self) -> &'static str {
        match self {
            Self::Function => "kids()?",
            Self::Static => "maybe_items?",
        }
    }

    fn dep_decl(self) -> &'static str {
        match self {
            Self::Function => "pub fn maybe_children() -> Option[[Child; 2]]",
            Self::Static => "pub static MAYBE_ITEMS: Option[[Child; 2]]",
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

fn build_dep_qi(member: MemberKind, root: RootKind) -> String {
    format!(
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {{
    value: Int,
}}

{root_decl}{member_impl}
"#,
        root_decl = root.dep_decl(),
        member_impl = member.dep_impl(),
    )
}

fn build_source(
    member: MemberKind,
    root: RootKind,
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
    for current in ({receiver}) {{
        return {access_expr}
    }}
{broken_line}    return 0
}}
"#,
        use_decl = root.use_decl(),
        receiver = structured.wrap(root.receiver_expr()),
        access_expr = member.access_expr(),
        broken_line = broken_line,
    )
}

fn run_definition_case(
    member: MemberKind,
    root: RootKind,
    structured: StructuredKind,
    broken: bool,
) {
    let temp = TempDir::new(&format!(
        "ql-lsp-for-loop-grouped-question-structured-iterable-{}-{}-{}-definition{}",
        member.label(),
        structured.label(),
        root.label(),
        if broken { "-broken" } else { "" }
    ));
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write("workspace/dep/dep.qi", &build_dep_qi(member, root));
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(member, root, structured, broken);
    temp.write("workspace/app/src/lib.ql", &source);

    let position = offset_to_position(&source, nth_offset(&source, member.token(), 1));
    let definition = if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        match member {
            MemberKind::Field => definition_for_dependency_struct_fields(&source, &package, position),
            MemberKind::Method => definition_for_dependency_methods(&source, &package, position),
        }
        .expect("grouped structured question iterable definition should exist")
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        match member {
            MemberKind::Field => definition_for_dependency_struct_fields(&source, &package, position),
            MemberKind::Method => definition_for_dependency_methods(&source, &package, position),
        }
        .expect("grouped structured question iterable definition should exist")
    };

    assert_targets_dependency_snippet(definition, &dep_qi, member.token());
}

#[test]
fn dependency_field_definition_works_on_for_loop_if_grouped_question_structured_iterable_function_receivers(
) {
    run_definition_case(MemberKind::Field, RootKind::Function, StructuredKind::If, false);
}

#[test]
fn dependency_field_definition_works_on_for_loop_if_grouped_question_structured_iterable_function_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Field, RootKind::Function, StructuredKind::If, true);
}

#[test]
fn dependency_field_definition_works_on_for_loop_if_grouped_question_structured_iterable_static_receivers(
) {
    run_definition_case(MemberKind::Field, RootKind::Static, StructuredKind::If, false);
}

#[test]
fn dependency_field_definition_works_on_for_loop_if_grouped_question_structured_iterable_static_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Field, RootKind::Static, StructuredKind::If, true);
}

#[test]
fn dependency_field_definition_works_on_for_loop_match_grouped_question_structured_iterable_function_receivers(
) {
    run_definition_case(MemberKind::Field, RootKind::Function, StructuredKind::Match, false);
}

#[test]
fn dependency_field_definition_works_on_for_loop_match_grouped_question_structured_iterable_function_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Field, RootKind::Function, StructuredKind::Match, true);
}

#[test]
fn dependency_field_definition_works_on_for_loop_match_grouped_question_structured_iterable_static_receivers(
) {
    run_definition_case(MemberKind::Field, RootKind::Static, StructuredKind::Match, false);
}

#[test]
fn dependency_field_definition_works_on_for_loop_match_grouped_question_structured_iterable_static_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Field, RootKind::Static, StructuredKind::Match, true);
}

#[test]
fn dependency_method_definition_works_on_for_loop_if_grouped_question_structured_iterable_function_receivers(
) {
    run_definition_case(MemberKind::Method, RootKind::Function, StructuredKind::If, false);
}

#[test]
fn dependency_method_definition_works_on_for_loop_if_grouped_question_structured_iterable_function_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Method, RootKind::Function, StructuredKind::If, true);
}

#[test]
fn dependency_method_definition_works_on_for_loop_if_grouped_question_structured_iterable_static_receivers(
) {
    run_definition_case(MemberKind::Method, RootKind::Static, StructuredKind::If, false);
}

#[test]
fn dependency_method_definition_works_on_for_loop_if_grouped_question_structured_iterable_static_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Method, RootKind::Static, StructuredKind::If, true);
}

#[test]
fn dependency_method_definition_works_on_for_loop_match_grouped_question_structured_iterable_function_receivers(
) {
    run_definition_case(MemberKind::Method, RootKind::Function, StructuredKind::Match, false);
}

#[test]
fn dependency_method_definition_works_on_for_loop_match_grouped_question_structured_iterable_function_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Method, RootKind::Function, StructuredKind::Match, true);
}

#[test]
fn dependency_method_definition_works_on_for_loop_match_grouped_question_structured_iterable_static_receivers(
) {
    run_definition_case(MemberKind::Method, RootKind::Static, StructuredKind::Match, false);
}

#[test]
fn dependency_method_definition_works_on_for_loop_match_grouped_question_structured_iterable_static_receivers_without_semantic_analysis(
) {
    run_definition_case(MemberKind::Method, RootKind::Static, StructuredKind::Match, true);
}
