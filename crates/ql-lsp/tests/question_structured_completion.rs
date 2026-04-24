use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_dependency_member_fields, completion_for_dependency_methods,
    completion_for_package_analysis,
};
use tower_lsp::lsp_types::{CompletionResponse, Position};

mod common;

use common::completion::{assert_member_completion_item, MemberKind};

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

fn member_receiver_expr(member: MemberKind) -> &'static str {
    match member {
        MemberKind::Field => "config.child?",
        MemberKind::Method => "config.child()?",
    }
}

fn member_dependency_qi(member: MemberKind) -> &'static str {
    match member {
        MemberKind::Field => {
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct Config {
    child: Option[Child],
}
"#
        }
        MemberKind::Method => {
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {
    id: Int,
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn get(self) -> Int
}
"#
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
            Self::Match => {
                format!("match flag {{\n        true => {expr},\n        false => {expr},\n    }}")
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

fn build_source(member: MemberKind, structured: StructuredKind, broken: bool) -> String {
    let broken_line = if broken {
        "    let broken: Int = \"oops\"\n"
    } else {
        ""
    };
    format!(
        r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg, flag: Bool) -> Int {{
{broken_line}    return ({wrapped}){suffix}
}}
"#,
        wrapped = structured.wrap(member_receiver_expr(member)),
        suffix = member.completion_suffix(),
    )
}

fn run_completion_case(member: MemberKind, structured: StructuredKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-{}-question-{}-completion{}",
        structured.label(),
        member.label(),
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
    temp.write("workspace/dep/dep.qi", member_dependency_qi(member));
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(member, structured, broken);
    temp.write("workspace/app/src/lib.ql", &source);
    let completion_offset =
        nth_offset(&source, member.completion_suffix(), 1) + member.completion_suffix().len();
    let position = offset_to_position(&source, completion_offset);

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let Some(CompletionResponse::Array(items)) = (match member {
            MemberKind::Field => {
                completion_for_dependency_member_fields(&source, &package, position)
            }
            MemberKind::Method => completion_for_dependency_methods(&source, &package, position),
        }) else {
            panic!(
                "structured question-unwrapped member completion should exist without semantic analysis"
            );
        };
        assert_eq!(items.len(), 1);
        assert_member_completion_item(member, &items[0]);
    } else {
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let analysis =
            analyze_source(&source).expect("analysis should succeed for completion query");
        let Some(CompletionResponse::Array(items)) =
            completion_for_package_analysis(&source, &analysis, &package, position)
        else {
            panic!("structured question-unwrapped member completion should exist");
        };
        assert_eq!(items.len(), 1);
        assert_member_completion_item(member, &items[0]);
    }
}

#[test]
fn dependency_field_completion_works_on_if_structured_question_unwrapped_receiver() {
    run_completion_case(MemberKind::Field, StructuredKind::If, false);
}

#[test]
fn dependency_field_completion_works_on_if_structured_question_unwrapped_receiver_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Field, StructuredKind::If, true);
}

#[test]
fn dependency_field_completion_works_on_match_structured_question_unwrapped_receiver() {
    run_completion_case(MemberKind::Field, StructuredKind::Match, false);
}

#[test]
fn dependency_field_completion_works_on_match_structured_question_unwrapped_receiver_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Field, StructuredKind::Match, true);
}

#[test]
fn dependency_method_completion_works_on_if_structured_question_unwrapped_receiver() {
    run_completion_case(MemberKind::Method, StructuredKind::If, false);
}

#[test]
fn dependency_method_completion_works_on_if_structured_question_unwrapped_receiver_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Method, StructuredKind::If, true);
}

#[test]
fn dependency_method_completion_works_on_match_structured_question_unwrapped_receiver() {
    run_completion_case(MemberKind::Method, StructuredKind::Match, false);
}

#[test]
fn dependency_method_completion_works_on_match_structured_question_unwrapped_receiver_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Method, StructuredKind::Match, true);
}
