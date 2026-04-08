use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    declaration_for_dependency_values, declaration_for_package_analysis,
    definition_for_dependency_values, definition_for_package_analysis, hover_for_dependency_values,
    hover_for_package_analysis, references_for_dependency_values, references_for_package_analysis,
    span_to_range,
};
use ql_span::Span;
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

    fn alias(self) -> &'static str {
        match self {
            Self::Function => "kids",
            Self::Static => "maybe_items",
        }
    }

    fn receiver_expr(self) -> &'static str {
        match self {
            Self::Function => "kids()?",
            Self::Static => "maybe_items?",
        }
    }

    fn dep_member_decl(self) -> &'static str {
        match self {
            Self::Function => "pub fn maybe_children() -> Option[[Child; 2]]",
            Self::Static => "pub static MAYBE_ITEMS: Option[[Child; 2]]",
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

fn dependency_qi(root: RootKind) -> String {
    format!(
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {{
    value: Int,
}}

{dep_member}"#,
        dep_member = root.dep_member_decl(),
    )
}

fn build_source(root: RootKind, broken: bool) -> String {
    let suffix = if broken { "    return \"oops\"\n" } else { "    return 0\n" };
    format!(
        r#"
package demo.app

{use_decl}

pub fn total() -> Int {{
    for current in {receiver} {{
        let first = current.value
    }}
    for current in {receiver} {{
        let second = current.value
    }}
{suffix}}}
"#,
        use_decl = root.use_decl(),
        receiver = root.receiver_expr(),
        suffix = suffix
    )
}

fn assert_dependency_location(location: &Location, dep_qi: &Path, snippet: &str) {
    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .expect("dependency snippet should exist");
    assert_eq!(
        location
            .uri
            .to_file_path()
            .expect("dependency URI should convert to a file path")
            .canonicalize()
            .expect("dependency URI path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert_eq!(
        location.range,
        span_to_range(&artifact, Span::new(start, start + snippet.len())),
    );
}

fn assert_root_queries(
    root: RootKind,
    source: &str,
    dep_qi: &Path,
    uri: &Url,
    broken: bool,
    app_root: &Path,
) {
    let alias = root.alias();
    let root_position = offset_to_position(source, nth_offset(source, alias, 3));

    if broken {
        assert!(analyze_package(app_root).is_err());
        let package = analyze_package_dependencies(app_root)
            .expect("dependency-only package analysis should succeed");

        let hover = hover_for_dependency_values(source, &package, root_position)
            .expect("grouped dependency question iterable root hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("**struct** `Child`"));
        assert!(markup.value.contains("struct Child"));

        let definition = definition_for_dependency_values(source, &package, root_position)
            .expect("grouped dependency question iterable root definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("definition should be one location")
        };
        assert_dependency_location(&location, dep_qi, "pub struct Child {\n    value: Int,\n}");

        let declaration = declaration_for_dependency_values(source, &package, root_position)
            .expect("grouped dependency question iterable root declaration should exist");
        let GotoDeclarationResponse::Scalar(location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_dependency_location(&location, dep_qi, "pub struct Child {\n    value: Int,\n}");

        let without_declaration =
            references_for_dependency_values(uri, source, &package, root_position, false)
                .expect("grouped dependency question iterable root references should exist");
        assert_eq!(
            without_declaration,
            vec![
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 2),
                            nth_offset(source, alias, 2) + alias.len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 3),
                            nth_offset(source, alias, 3) + alias.len(),
                        ),
                    ),
                ),
            ]
        );

        let with_declaration =
            references_for_dependency_values(uri, source, &package, root_position, true).expect(
                "grouped dependency question iterable root references with declaration should exist",
            );
        assert_eq!(with_declaration.len(), 4);
        assert_dependency_location(
            &with_declaration[0],
            dep_qi,
            "pub struct Child {\n    value: Int,\n}",
        );
        assert_eq!(
            with_declaration[1..],
            [
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 1),
                            nth_offset(source, alias, 1) + alias.len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 2),
                            nth_offset(source, alias, 2) + alias.len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 3),
                            nth_offset(source, alias, 3) + alias.len(),
                        ),
                    ),
                ),
            ]
        );
    } else {
        let package = analyze_package(app_root).expect("package analysis should succeed");
        let analysis = analyze_source(source).expect("source should analyze");

        let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
            .expect("grouped dependency question iterable root hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("**struct** `Child`"));
        assert!(markup.value.contains("struct Child"));

        let definition =
            definition_for_package_analysis(uri, source, &analysis, &package, root_position)
                .expect("grouped dependency question iterable root definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("definition should be one location")
        };
        assert_dependency_location(&location, dep_qi, "pub struct Child {\n    value: Int,\n}");

        let declaration =
            declaration_for_package_analysis(uri, source, &analysis, &package, root_position)
                .expect("grouped dependency question iterable root declaration should exist");
        let GotoDeclarationResponse::Scalar(location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_dependency_location(&location, dep_qi, "pub struct Child {\n    value: Int,\n}");

        let without_declaration =
            references_for_package_analysis(uri, source, &analysis, &package, root_position, false)
                .expect("grouped dependency question iterable root references should exist");
        assert_eq!(
            without_declaration,
            vec![
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 2),
                            nth_offset(source, alias, 2) + alias.len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 3),
                            nth_offset(source, alias, 3) + alias.len(),
                        ),
                    ),
                ),
            ]
        );

        let with_declaration = references_for_package_analysis(
            uri,
            source,
            &analysis,
            &package,
            root_position,
            true,
        )
        .expect("grouped dependency question iterable root references with declaration should exist");
        assert_eq!(with_declaration.len(), 4);
        assert_dependency_location(
            &with_declaration[0],
            dep_qi,
            "pub struct Child {\n    value: Int,\n}",
        );
        assert_eq!(
            with_declaration[1..],
            [
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 1),
                            nth_offset(source, alias, 1) + alias.len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 2),
                            nth_offset(source, alias, 2) + alias.len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        source,
                        Span::new(
                            nth_offset(source, alias, 3),
                            nth_offset(source, alias, 3) + alias.len(),
                        ),
                    ),
                ),
            ]
        );
    }
}

fn run_root_query_case(root: RootKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-grouped-question-iterable-{}-root-query{}",
        root.label(),
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
    let dep_qi = temp.write("workspace/dep/dep.qi", &dependency_qi(root));
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(root, broken);
    temp.write("workspace/app/src/lib.ql", &source);

    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    assert_root_queries(root, &source, &dep_qi, &uri, broken, &app_root);
}

#[test]
fn root_queries_work_on_grouped_question_function_iterable_roots() {
    run_root_query_case(RootKind::Function, false);
}

#[test]
fn root_queries_work_on_grouped_question_function_iterable_roots_without_semantic_analysis() {
    run_root_query_case(RootKind::Function, true);
}

#[test]
fn root_queries_work_on_grouped_question_static_iterable_roots() {
    run_root_query_case(RootKind::Static, false);
}

#[test]
fn root_queries_work_on_grouped_question_static_iterable_roots_without_semantic_analysis() {
    run_root_query_case(RootKind::Static, true);
}
