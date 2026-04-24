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

    fn dep_qi(self) -> &'static str {
        match self {
            Self::Function => {
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
"#
            }
            Self::Static => {
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
"#
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

fn build_source(member: MemberKind, root: RootKind, broken: bool) -> String {
    let broken_line = if broken {
        "    let broken: Int = \"oops\"\n"
    } else {
        ""
    };
    format!(
        r#"
package demo.app

{use_decl}

pub fn read() -> Int {{
    for current in {receiver} {{
        let value = current{suffix}
    }}
{broken_line}    return 0
}}
"#,
        use_decl = root.use_decl(),
        receiver = root.receiver_expr(),
        suffix = member.completion_suffix(),
        broken_line = broken_line,
    )
}

fn run_completion_case(member: MemberKind, root: RootKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-for-loop-grouped-question-iterable-{}-{}-completion{}",
        member.label(),
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
    temp.write("workspace/dep/dep.qi", root.dep_qi());
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(member, root, broken);
    temp.write("workspace/app/src/lib.ql", &source);

    let position = offset_to_position(
        &source,
        nth_offset(&source, member.completion_suffix(), 1) + member.completion_suffix().len(),
    );

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
                "grouped question iterable member completion should exist without semantic analysis"
            );
        };
        assert_eq!(items.len(), 1);
        assert_member_completion_item(member, &items[0]);
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis =
            analyze_source(&source).expect("analysis should succeed for completion query");
        let Some(CompletionResponse::Array(items)) =
            completion_for_package_analysis(&source, &analysis, &package, position)
        else {
            panic!("grouped question iterable member completion should exist");
        };
        assert_eq!(items.len(), 1);
        assert_member_completion_item(member, &items[0]);
    }
}

#[test]
fn dependency_field_completion_works_on_for_loop_grouped_question_function_iterables() {
    run_completion_case(MemberKind::Field, RootKind::Function, false);
}

#[test]
fn dependency_field_completion_works_on_for_loop_grouped_question_function_iterables_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Field, RootKind::Function, true);
}

#[test]
fn dependency_field_completion_works_on_for_loop_grouped_question_static_iterables() {
    run_completion_case(MemberKind::Field, RootKind::Static, false);
}

#[test]
fn dependency_field_completion_works_on_for_loop_grouped_question_static_iterables_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Field, RootKind::Static, true);
}

#[test]
fn dependency_method_completion_works_on_for_loop_grouped_question_function_iterables() {
    run_completion_case(MemberKind::Method, RootKind::Function, false);
}

#[test]
fn dependency_method_completion_works_on_for_loop_grouped_question_function_iterables_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Method, RootKind::Function, true);
}

#[test]
fn dependency_method_completion_works_on_for_loop_grouped_question_static_iterables() {
    run_completion_case(MemberKind::Method, RootKind::Static, false);
}

#[test]
fn dependency_method_completion_works_on_for_loop_grouped_question_static_iterables_without_semantic_analysis(
) {
    run_completion_case(MemberKind::Method, RootKind::Static, true);
}
