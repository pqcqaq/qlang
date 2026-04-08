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
enum CaseKind {
    UnsafeTupleField,
    UnsafeArrayMethod,
}

impl CaseKind {
    fn label(self) -> &'static str {
        match self {
            Self::UnsafeTupleField => "unsafe-tuple-field",
            Self::UnsafeArrayMethod => "unsafe-array-method",
        }
    }

    fn member_token(self) -> &'static str {
        "leaf"
    }

    fn access_suffix(self) -> &'static str {
        match self {
            Self::UnsafeTupleField => ".leaf.value",
            Self::UnsafeArrayMethod => ".leaf().value",
        }
    }

    fn build_source(self, broken: bool) -> String {
        let broken_line = if broken {
            "    let broken: Int = \"oops\"\n"
        } else {
            ""
        };
        match self {
            Self::UnsafeTupleField => format!(
                r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {{
    for current in unsafe {{ (config, config) }} {{
        return current{suffix}
    }}
{broken_line}    return 0
}}
"#,
                suffix = self.access_suffix(),
                broken_line = broken_line,
            ),
            Self::UnsafeArrayMethod => format!(
                r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {{
    for current in unsafe {{ [config.child(), config.child()] }} {{
        return current{suffix}
    }}
{broken_line}    return 0
}}
"#,
                suffix = self.access_suffix(),
                broken_line = broken_line,
            ),
        }
    }
}

fn dependency_qi(case: CaseKind) -> &'static str {
    match case {
        CaseKind::UnsafeTupleField => {
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Config {
    leaf: Leaf,
}
"#
        }
        CaseKind::UnsafeArrayMethod => {
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

pub struct Config {
    id: Int,
}

impl Config {
    pub fn child(self) -> Child
}

impl Child {
    pub fn leaf(self) -> Leaf
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

fn run_type_definition_case(case: CaseKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-for-loop-unsafe-iterable-{}-member-type-definition{}",
        case.label(),
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
    let dep_qi = temp.write("workspace/dep/dep.qi", dependency_qi(case));
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
    let position = offset_to_position(&source, nth_offset(&source, case.member_token(), 1));

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let definition = match case {
            CaseKind::UnsafeTupleField => {
                type_definition_for_dependency_struct_field_types(&source, &package, position)
            }
            CaseKind::UnsafeArrayMethod => {
                type_definition_for_dependency_method_types(&source, &package, position)
            }
        }
        .expect("unsafe iterable member type definition should exist without semantic analysis");
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
                .expect("unsafe iterable member type definition should exist");
        assert_targets_dependency_type(
            definition,
            &dep_qi,
            "pub struct Leaf {\n    value: Int,\n}",
        );
    }
}

#[test]
fn type_definition_bridge_follows_unsafe_tuple_for_loop_field_member_types() {
    run_type_definition_case(CaseKind::UnsafeTupleField, false);
}

#[test]
fn type_definition_fallback_follows_unsafe_tuple_for_loop_field_member_types() {
    run_type_definition_case(CaseKind::UnsafeTupleField, true);
}

#[test]
fn type_definition_bridge_follows_unsafe_array_for_loop_method_member_types() {
    run_type_definition_case(CaseKind::UnsafeArrayMethod, false);
}

#[test]
fn type_definition_fallback_follows_unsafe_array_for_loop_method_member_types() {
    run_type_definition_case(CaseKind::UnsafeArrayMethod, true);
}
