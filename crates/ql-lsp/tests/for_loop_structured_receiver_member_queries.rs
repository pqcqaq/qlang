use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies};
use ql_lsp::bridge::{
    declaration_for_dependency_methods, declaration_for_dependency_struct_fields,
    definition_for_dependency_methods, definition_for_dependency_struct_fields,
    hover_for_dependency_methods, hover_for_dependency_struct_fields,
    references_for_dependency_methods, references_for_dependency_struct_fields, span_to_range,
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

    fn member_token(self) -> &'static str {
        match self {
            Self::IfTupleField => "value",
            Self::MatchArrayMethod => "get",
        }
    }

    fn access_suffix(self) -> &'static str {
        match self {
            Self::IfTupleField => ".value",
            Self::MatchArrayMethod => ".get()",
        }
    }

    fn hover_kind(self) -> &'static str {
        match self {
            Self::IfTupleField => "**field** `value`",
            Self::MatchArrayMethod => "**method** `get`",
        }
    }

    fn hover_detail(self) -> &'static str {
        match self {
            Self::IfTupleField => "field value: Int",
            Self::MatchArrayMethod => "fn get(self) -> Int",
        }
    }

    fn declaration_anchor(self) -> &'static str {
        match self {
            Self::IfTupleField => "value: Int",
            Self::MatchArrayMethod => "pub fn get(self) -> Int",
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
        let first = current{suffix}
        let second = current{suffix}
        return first + second
    }}
{broken_line}    return 0
}}
"#,
                suffix = self.access_suffix(),
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
        let first = current{suffix}
        let second = current{suffix}
        return first + second
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

fn assert_member_queries(
    case: CaseKind,
    source: &str,
    dep_qi: &Path,
    uri: &Url,
    first_offset: usize,
    second_offset: usize,
    broken: bool,
    app_root: &Path,
) {
    if broken {
        assert!(analyze_package(app_root).is_err());
        let package = analyze_package_dependencies(app_root)
            .expect("dependency-only package analysis should succeed");

        let hover = match case {
            CaseKind::IfTupleField => hover_for_dependency_struct_fields(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
            CaseKind::MatchArrayMethod => hover_for_dependency_methods(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
        }
        .expect("structured iterable member hover should exist without semantic analysis");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains(case.hover_kind()));
        assert!(markup.value.contains(case.hover_detail()));

        let definition = match case {
            CaseKind::IfTupleField => definition_for_dependency_struct_fields(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
            CaseKind::MatchArrayMethod => definition_for_dependency_methods(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
        }
        .expect("structured iterable member definition should exist without semantic analysis");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("definition should be one location")
        };
        assert_location_targets_dependency_name(
            &definition_location,
            dep_qi,
            case.declaration_anchor(),
            case.member_token(),
        );

        let declaration = match case {
            CaseKind::IfTupleField => declaration_for_dependency_struct_fields(
                source,
                &package,
                offset_to_position(source, second_offset),
            ),
            CaseKind::MatchArrayMethod => declaration_for_dependency_methods(
                source,
                &package,
                offset_to_position(source, second_offset),
            ),
        }
        .expect("structured iterable member declaration should exist without semantic analysis");
        let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_location_targets_dependency_name(
            &declaration_location,
            dep_qi,
            case.declaration_anchor(),
            case.member_token(),
        );

        let with_declaration = match case {
            CaseKind::IfTupleField => references_for_dependency_struct_fields(
                uri,
                source,
                &package,
                offset_to_position(source, second_offset),
                true,
            ),
            CaseKind::MatchArrayMethod => references_for_dependency_methods(
                uri,
                source,
                &package,
                offset_to_position(source, second_offset),
                true,
            ),
        }
        .expect("structured iterable member references should exist without semantic analysis");
        assert_eq!(with_declaration.len(), 3);
        assert_location_targets_dependency_name(
            &with_declaration[0],
            dep_qi,
            case.declaration_anchor(),
            case.member_token(),
        );

        let without_declaration = match case {
            CaseKind::IfTupleField => references_for_dependency_struct_fields(
                uri,
                source,
                &package,
                offset_to_position(source, first_offset),
                false,
            ),
            CaseKind::MatchArrayMethod => references_for_dependency_methods(
                uri,
                source,
                &package,
                offset_to_position(source, first_offset),
                false,
            ),
        }
        .expect("structured iterable member references should exist without declaration");
        assert_eq!(without_declaration.len(), 2);
        assert!(without_declaration
            .iter()
            .all(|location| location.uri == *uri));

        let expected_first = span_to_range(
            source,
            ql_span::Span::new(first_offset, first_offset + case.member_token().len()),
        );
        let expected_second = span_to_range(
            source,
            ql_span::Span::new(second_offset, second_offset + case.member_token().len()),
        );
        let with_local_ranges = with_declaration[1..]
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>();
        assert!(with_local_ranges.contains(&expected_first));
        assert!(with_local_ranges.contains(&expected_second));

        let local_ranges = without_declaration
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>();
        assert!(local_ranges.contains(&expected_first));
        assert!(local_ranges.contains(&expected_second));
    } else {
        let package = analyze_package(app_root).expect("package analysis should succeed");

        let hover = match case {
            CaseKind::IfTupleField => hover_for_dependency_struct_fields(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
            CaseKind::MatchArrayMethod => hover_for_dependency_methods(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
        }
        .expect("structured iterable member hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains(case.hover_kind()));
        assert!(markup.value.contains(case.hover_detail()));

        let definition = match case {
            CaseKind::IfTupleField => definition_for_dependency_struct_fields(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
            CaseKind::MatchArrayMethod => definition_for_dependency_methods(
                source,
                &package,
                offset_to_position(source, first_offset),
            ),
        }
        .expect("structured iterable member definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("definition should be one location")
        };
        assert_location_targets_dependency_name(
            &definition_location,
            dep_qi,
            case.declaration_anchor(),
            case.member_token(),
        );

        let declaration = match case {
            CaseKind::IfTupleField => declaration_for_dependency_struct_fields(
                source,
                &package,
                offset_to_position(source, second_offset),
            ),
            CaseKind::MatchArrayMethod => declaration_for_dependency_methods(
                source,
                &package,
                offset_to_position(source, second_offset),
            ),
        }
        .expect("structured iterable member declaration should exist");
        let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_location_targets_dependency_name(
            &declaration_location,
            dep_qi,
            case.declaration_anchor(),
            case.member_token(),
        );

        let with_declaration = match case {
            CaseKind::IfTupleField => references_for_dependency_struct_fields(
                uri,
                source,
                &package,
                offset_to_position(source, second_offset),
                true,
            ),
            CaseKind::MatchArrayMethod => references_for_dependency_methods(
                uri,
                source,
                &package,
                offset_to_position(source, second_offset),
                true,
            ),
        }
        .expect("structured iterable member references should exist");
        assert_eq!(with_declaration.len(), 3);
        assert_location_targets_dependency_name(
            &with_declaration[0],
            dep_qi,
            case.declaration_anchor(),
            case.member_token(),
        );

        let without_declaration = match case {
            CaseKind::IfTupleField => references_for_dependency_struct_fields(
                uri,
                source,
                &package,
                offset_to_position(source, first_offset),
                false,
            ),
            CaseKind::MatchArrayMethod => references_for_dependency_methods(
                uri,
                source,
                &package,
                offset_to_position(source, first_offset),
                false,
            ),
        }
        .expect("structured iterable member references should exist without declaration");
        assert_eq!(without_declaration.len(), 2);
        assert!(without_declaration
            .iter()
            .all(|location| location.uri == *uri));

        let expected_first = span_to_range(
            source,
            ql_span::Span::new(first_offset, first_offset + case.member_token().len()),
        );
        let expected_second = span_to_range(
            source,
            ql_span::Span::new(second_offset, second_offset + case.member_token().len()),
        );
        let with_local_ranges = with_declaration[1..]
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>();
        assert!(with_local_ranges.contains(&expected_first));
        assert!(with_local_ranges.contains(&expected_second));

        let local_ranges = without_declaration
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>();
        assert!(local_ranges.contains(&expected_first));
        assert!(local_ranges.contains(&expected_second));
    }
}

fn run_member_query_case(case: CaseKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-for-loop-structured-receiver-{}-queries{}",
        case.label(),
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
    let app_file = temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_file).expect("test file path should convert to URL");

    assert_member_queries(
        case,
        &source,
        &dep_qi,
        &uri,
        nth_offset(&source, case.member_token(), 1),
        nth_offset(&source, case.member_token(), 2),
        broken,
        &app_root,
    );
}

#[test]
fn dependency_field_queries_work_on_for_loop_if_tuple_receivers() {
    run_member_query_case(CaseKind::IfTupleField, false);
}

#[test]
fn dependency_field_queries_work_on_for_loop_if_tuple_receivers_without_semantic_analysis() {
    run_member_query_case(CaseKind::IfTupleField, true);
}

#[test]
fn dependency_method_queries_work_on_for_loop_match_array_receivers() {
    run_member_query_case(CaseKind::MatchArrayMethod, false);
}

#[test]
fn dependency_method_queries_work_on_for_loop_match_array_receivers_without_semantic_analysis() {
    run_member_query_case(CaseKind::MatchArrayMethod, true);
}
