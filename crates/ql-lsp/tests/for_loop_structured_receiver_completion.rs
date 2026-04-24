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
enum CaseKind {
    IfTupleField,
    MatchArrayMethod,
}

impl CaseKind {
    fn label(self) -> &'static str {
        match self {
            Self::IfTupleField => "if-tuple-field",
            Self::MatchArrayMethod => "match-array-method",
        }
    }

    fn completion_suffix(self) -> &'static str {
        self.member_kind().completion_suffix()
    }

    fn member_kind(self) -> MemberKind {
        match self {
            Self::IfTupleField => MemberKind::Field,
            Self::MatchArrayMethod => MemberKind::Method,
        }
    }

    fn build_source(self, broken: bool) -> String {
        let broken_line = if broken {
            "    let broken: Int = \"oops\"\n"
        } else {
            ""
        };
        match self {
            Self::IfTupleField => format!(
                r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg, flag: Bool) -> Int {{
    for current in (if flag {{ (config, config) }} else {{ (config, config) }}) {{
        let value = current{suffix}
    }}
{broken_line}    return 0
}}
"#,
                suffix = self.completion_suffix(),
                broken_line = broken_line,
            ),
            Self::MatchArrayMethod => format!(
                r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg, flag: Bool) -> Int {{
    for current in match flag {{
        true => [config.child(), config.child()],
        false => [config.child(), config.child()],
    }} {{
        let value = current{suffix}
    }}
{broken_line}    return 0
}}
"#,
                suffix = self.completion_suffix(),
                broken_line = broken_line,
            ),
        }
    }
}

fn dependency_qi(case: CaseKind) -> &'static str {
    match case {
        CaseKind::IfTupleField => {
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#
        }
        CaseKind::MatchArrayMethod => {
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct Config {
    id: Int,
}

impl Config {
    pub fn child(self) -> Child
}

impl Child {
    pub fn get(self) -> Int
}
"#
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

fn run_completion_case(case: CaseKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-for-loop-structured-receiver-{}-{}-completion{}",
        case.label(),
        case.member_kind().label(),
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
    temp.write("workspace/dep/dep.qi", dependency_qi(case));
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = case.build_source(broken);
    temp.write("workspace/app/src/lib.ql", &source);

    let position = offset_to_position(
        &source,
        nth_offset(&source, case.completion_suffix(), 1) + case.completion_suffix().len(),
    );

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let Some(CompletionResponse::Array(items)) = (match case {
            CaseKind::IfTupleField => {
                completion_for_dependency_member_fields(&source, &package, position)
            }
            CaseKind::MatchArrayMethod => {
                completion_for_dependency_methods(&source, &package, position)
            }
        }) else {
            panic!(
                "structured iterable receiver completion should exist without semantic analysis"
            );
        };
        assert_eq!(items.len(), 1);
        assert_member_completion_item(case.member_kind(), &items[0]);
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis =
            analyze_source(&source).expect("analysis should succeed for completion query");
        let Some(CompletionResponse::Array(items)) =
            completion_for_package_analysis(&source, &analysis, &package, position)
        else {
            panic!("structured iterable receiver completion should exist");
        };
        assert_eq!(items.len(), 1);
        assert_member_completion_item(case.member_kind(), &items[0]);
    }
}

#[test]
fn dependency_field_completion_works_on_if_tuple_for_loop_receivers() {
    run_completion_case(CaseKind::IfTupleField, false);
}

#[test]
fn dependency_field_completion_works_on_if_tuple_for_loop_receivers_without_semantic_analysis() {
    run_completion_case(CaseKind::IfTupleField, true);
}

#[test]
fn dependency_method_completion_works_on_match_array_for_loop_receivers() {
    run_completion_case(CaseKind::MatchArrayMethod, false);
}

#[test]
fn dependency_method_completion_works_on_match_array_for_loop_receivers_without_semantic_analysis()
{
    run_completion_case(CaseKind::MatchArrayMethod, true);
}
