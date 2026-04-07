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

#[test]
fn root_query_bridge_surfaces_dependency_function_return_roots() {
    let temp = TempDir::new("ql-lsp-function-root-queries");
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

pub struct Config {
    value: Int,
}

pub fn load() -> Config
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

use demo.dep.load

pub fn main() -> Int {
    return load().value + load().value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "load", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("dependency function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load", 2),
                        nth_offset(source, "load", 2) + "load".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load", 3),
                        nth_offset(source, "load", 3) + "load".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("dependency function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load", 1),
                        nth_offset(source, "load", 1) + "load".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load", 2),
                        nth_offset(source, "load", 2) + "load".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load", 3),
                        nth_offset(source, "load", 3) + "load".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_question_wrapped_dependency_static_roots() {
    let temp = TempDir::new("ql-lsp-question-static-root-queries");
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

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.MAYBE as cfg

pub fn main() -> Int {
    return cfg?.value + cfg?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "cfg", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("dependency static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 2),
                        nth_offset(source, "cfg", 2) + "cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 3),
                        nth_offset(source, "cfg", 3) + "cfg".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("dependency static root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 1),
                        nth_offset(source, "cfg", 1) + "cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 2),
                        nth_offset(source, "cfg", 2) + "cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 3),
                        nth_offset(source, "cfg", 3) + "cfg".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_question_wrapped_dependency_function_roots() {
    let temp = TempDir::new("ql-lsp-question-function-root-queries");
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

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.maybe_load

pub fn main() -> Int {
    return maybe_load()?.value + maybe_load()?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_load", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency question function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency question function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency question function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("dependency question function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 2),
                        nth_offset(source, "maybe_load", 2) + "maybe_load".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 3),
                        nth_offset(source, "maybe_load", 3) + "maybe_load".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("dependency question function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 1),
                        nth_offset(source, "maybe_load", 1) + "maybe_load".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 2),
                        nth_offset(source, "maybe_load", 2) + "maybe_load".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 3),
                        nth_offset(source, "maybe_load", 3) + "maybe_load".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_dependency_const_roots_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-const-root-queries-broken");
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

pub struct Config {
    value: Int,
}

pub const DEFAULT: Config
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

use demo.dep.DEFAULT as cfg

pub fn main() -> Int {
    let next = cfg.value + cfg.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "cfg", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("dependency const root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("dependency const root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("dependency const root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 2),
                        nth_offset(source, "cfg", 2) + "cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 3),
                        nth_offset(source, "cfg", 3) + "cfg".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("dependency const root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 1),
                        nth_offset(source, "cfg", 1) + "cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 2),
                        nth_offset(source, "cfg", 2) + "cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "cfg", 3),
                        nth_offset(source, "cfg", 3) + "cfg".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_question_wrapped_dependency_static_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-static-root-queries-broken");
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

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.MAYBE as child

pub fn main() -> Int {
    let next = child?.value + child?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "child", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("dependency question static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("dependency question static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("dependency question static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("dependency question static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "child", 2),
                        nth_offset(source, "child", 2) + "child".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "child", 3),
                        nth_offset(source, "child", 3) + "child".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("dependency question static root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "child", 1),
                        nth_offset(source, "child", 1) + "child".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "child", 2),
                        nth_offset(source, "child", 2) + "child".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "child", 3),
                        nth_offset(source, "child", 3) + "child".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_question_wrapped_dependency_function_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-function-root-queries-broken");
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

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.maybe_load

pub fn main() -> Int {
    let next = maybe_load()?.value + maybe_load()?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_load", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("dependency question function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("dependency question function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("dependency question function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("dependency question function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 2),
                        nth_offset(source, "maybe_load", 2) + "maybe_load".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 3),
                        nth_offset(source, "maybe_load", 3) + "maybe_load".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("dependency question function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 1),
                        nth_offset(source, "maybe_load", 1) + "maybe_load".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 2),
                        nth_offset(source, "maybe_load", 2) + "maybe_load".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_load", 3),
                        nth_offset(source, "maybe_load", 3) + "maybe_load".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-function-iterable-root-queries");
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

pub fn children() -> [Child; 2]
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

use demo.dep.children

pub fn total() -> Int {
    for current in children() {
        let first = current.value
    }
    for current in children() {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "children", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("dependency iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "children", 2),
                        nth_offset(source, "children", 2) + "children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "children", 3),
                        nth_offset(source, "children", 3) + "children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("dependency iterable function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "children", 1),
                        nth_offset(source, "children", 1) + "children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "children", 2),
                        nth_offset(source, "children", 2) + "children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "children", 3),
                        nth_offset(source, "children", 3) + "children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_dependency_const_iterable_roots() {
    let temp = TempDir::new("ql-lsp-const-iterable-root-queries");
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

pub const ITEMS: [Child; 2]
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

use demo.dep.ITEMS

pub fn total() -> Int {
    for current in ITEMS {
        let first = current.value
    }
    for current in ITEMS {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "ITEMS", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency iterable const root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency iterable const root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency iterable const root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("dependency iterable const root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 2),
                        nth_offset(source, "ITEMS", 2) + "ITEMS".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 3),
                        nth_offset(source, "ITEMS", 3) + "ITEMS".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("dependency iterable const root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "ITEMS", 1),
                        nth_offset(source, "ITEMS", 1) + "ITEMS".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 2),
                        nth_offset(source, "ITEMS", 2) + "ITEMS".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 3),
                        nth_offset(source, "ITEMS", 3) + "ITEMS".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_dependency_const_iterable_roots_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-const-iterable-root-queries-broken");
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

pub const ITEMS: [Child; 2]
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

use demo.dep.ITEMS

pub fn total() -> Int {
    for current in ITEMS {
        let first = current.value
    }
    for current in ITEMS {
        let second = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "ITEMS", 3));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("dependency iterable const root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("dependency iterable const root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("dependency iterable const root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("dependency iterable const root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 2),
                        nth_offset(source, "ITEMS", 2) + "ITEMS".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 3),
                        nth_offset(source, "ITEMS", 3) + "ITEMS".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("dependency iterable const root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "ITEMS", 1),
                        nth_offset(source, "ITEMS", 1) + "ITEMS".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 2),
                        nth_offset(source, "ITEMS", 2) + "ITEMS".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "ITEMS", 3),
                        nth_offset(source, "ITEMS", 3) + "ITEMS".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-function-iterable-root-queries");
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

pub fn children() -> [Child; 2]
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

use demo.dep.{children as kids}

pub fn total() -> Int {
    for current in kids() {
        let first = current.value
    }
    for current in kids() {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "kids", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped dependency iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("grouped dependency iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect("grouped dependency iterable function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "kids", 1),
                        nth_offset(source, "kids", 1) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_dependency_const_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-const-iterable-root-queries");
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

pub const ITEMS: [Child; 2]
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

use demo.dep.{ITEMS as items}

pub fn total() -> Int {
    for current in items {
        let first = current.value
    }
    for current in items {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "items", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped dependency iterable const root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency iterable const root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency iterable const root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("grouped dependency iterable const root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect("grouped dependency iterable const root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "items", 1),
                        nth_offset(source, "items", 1) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_grouped_import_dependency_const_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-const-iterable-root-queries-broken");
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

pub const ITEMS: [Child; 2]
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

use demo.dep.{ITEMS as items}

pub fn total() -> Int {
    for current in items {
        let first = current.value
    }
    for current in items {
        let second = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "items", 3));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency iterable const root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency iterable const root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency iterable const root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("grouped dependency iterable const root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("grouped dependency iterable const root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "items", 1),
                        nth_offset(source, "items", 1) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_question_wrapped_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-question-function-iterable-root-queries");
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

use demo.dep.maybe_children

pub fn total() -> Int {
    for current in maybe_children()? {
        let first = current.value
    }
    for current in maybe_children()? {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_children", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("dependency question iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect(
        "dependency question iterable function root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_children", 1),
                        nth_offset(source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_question_wrapped_dependency_static_iterable_roots() {
    let temp = TempDir::new("ql-lsp-question-static-iterable-root-queries");
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

use demo.dep.MAYBE_ITEMS as items

pub fn total() -> Int {
    for current in items? {
        let first = current.value
    }
    for current in items? {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "items", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("dependency question iterable static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency question iterable static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("dependency question iterable static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("dependency question iterable static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect("dependency question iterable static root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "items", 1),
                        nth_offset(source, "items", 1) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_structured_question_wrapped_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-structured-question-function-iterable-root-queries");
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

use demo.dep.maybe_children

pub fn total(flag: Bool) -> Int {
    for current in (if flag { maybe_children()? } else { maybe_children()? }) {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_children", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("structured dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("structured dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("structured dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("structured dependency question iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect(
        "structured dependency question iterable function root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_children", 1),
                        nth_offset(source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_match_structured_question_wrapped_dependency_function_iterable_roots(
) {
    let temp = TempDir::new("ql-lsp-match-structured-question-function-iterable-root-queries");
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

use demo.dep.maybe_children

pub fn total(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    } {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_children", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("match structured dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position).expect(
            "match structured dependency question iterable function root definition should exist",
        );
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position).expect(
            "match structured dependency question iterable function root declaration should exist",
        );
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("match structured dependency question iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect(
        "match structured dependency question iterable function root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_children", 1),
                        nth_offset(source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-function-iterable-root-queries-broken");
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

use demo.dep.maybe_children

pub fn total() -> Int {
    for current in maybe_children()? {
        let first = current.value
    }
    for current in maybe_children()? {
        let second = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_children", 3));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("dependency question iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect(
                "dependency question iterable function root references with declaration should exist",
            );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_children", 1),
                        nth_offset(source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-structured-question-function-iterable-root-queries-broken");
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

use demo.dep.maybe_children

pub fn total(flag: Bool) -> Int {
    for current in (if flag { maybe_children()? } else { maybe_children()? }) {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_children", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("structured dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("structured dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("structured dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect(
                "structured dependency question iterable function root references should exist",
            );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect(
                "structured dependency question iterable function root references with declaration should exist",
            );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_children", 1),
                        nth_offset(source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_match_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp =
        TempDir::new("ql-lsp-match-structured-question-function-iterable-root-queries-broken");
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

use demo.dep.maybe_children

pub fn total(flag: Bool) -> Int {
    for current in match flag {
        true => maybe_children()?,
        false => maybe_children()?,
    } {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_children", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("match structured dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("match structured dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("match structured dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false).expect(
            "match structured dependency question iterable function root references should exist",
        );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true).expect(
            "match structured dependency question iterable function root references with declaration should exist",
        );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_children", 1),
                        nth_offset(source, "maybe_children", 1) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 2),
                        nth_offset(source, "maybe_children", 2) + "maybe_children".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_children", 3),
                        nth_offset(source, "maybe_children", 3) + "maybe_children".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_question_wrapped_dependency_static_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-question-static-iterable-root-queries-broken");
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

use demo.dep.MAYBE_ITEMS as items

pub fn total() -> Int {
    for current in items? {
        let first = current.value
    }
    for current in items? {
        let second = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "items", 3));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("dependency question iterable static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("dependency question iterable static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("dependency question iterable static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("dependency question iterable static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect(
                "dependency question iterable static root references with declaration should exist",
            );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "items", 1),
                        nth_offset(source, "items", 1) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 2),
                        nth_offset(source, "items", 2) + "items".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "items", 3),
                        nth_offset(source, "items", 3) + "items".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_question_wrapped_dependency_function_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-function-iterable-root-queries");
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

pub fn total() -> Int {
    for current in kids()? {
        let first = current.value
    }
    for current in kids()? {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "kids", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("grouped dependency question iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect(
        "grouped dependency question iterable function root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "kids", 1),
                        nth_offset(source, "kids", 1) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_match_structured_question_wrapped_dependency_function_iterable_roots(
) {
    let temp = TempDir::new("ql-lsp-grouped-match-question-function-iterable-root-queries");
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

pub fn total(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let first = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "kids", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped match structured dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect(
                "grouped match structured dependency question iterable function root definition should exist",
            );
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect(
                "grouped match structured dependency question iterable function root declaration should exist",
            );
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect(
        "grouped match structured dependency question iterable function root references should exist",
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect(
        "grouped match structured dependency question iterable function root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "kids", 1),
                        nth_offset(source, "kids", 1) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_question_wrapped_dependency_static_iterable_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-static-iterable-root-queries");
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

pub fn total() -> Int {
    for current in maybe_items? {
        let first = current.value
    }
    for current in maybe_items? {
        let second = current.value
    }
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_items", 3));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped dependency question iterable static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question iterable static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question iterable static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        false,
    )
    .expect("grouped dependency question iterable static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 2),
                        nth_offset(source, "maybe_items", 2) + "maybe_items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 3),
                        nth_offset(source, "maybe_items", 3) + "maybe_items".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        root_position,
        true,
    )
    .expect(
        "grouped dependency question iterable static root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_items", 1),
                        nth_offset(source, "maybe_items", 1) + "maybe_items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 2),
                        nth_offset(source, "maybe_items", 2) + "maybe_items".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 3),
                        nth_offset(source, "maybe_items", 3) + "maybe_items".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_grouped_import_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-function-iterable-root-queries-broken");
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

pub fn total() -> Int {
    for current in kids()? {
        let first = current.value
    }
    for current in kids()? {
        let second = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "kids", 3));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question iterable function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question iterable function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question iterable function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("grouped dependency question iterable function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect(
                "grouped dependency question iterable function root references with declaration should exist",
            );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "kids", 1),
                        nth_offset(source, "kids", 1) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_grouped_import_match_structured_question_wrapped_dependency_function_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new(
        "ql-lsp-grouped-match-question-function-iterable-root-queries-broken",
    );
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

pub fn total(flag: Bool) -> Int {
    for current in match flag {
        true => kids()?,
        false => kids()?,
    } {
        let first = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "kids", 2));
    let hover = hover_for_dependency_values(source, &package, root_position).expect(
        "grouped match structured dependency question iterable function root hover should exist",
    );
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position).expect(
        "grouped match structured dependency question iterable function root definition should exist",
    );
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position).expect(
        "grouped match structured dependency question iterable function root declaration should exist",
    );
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false).expect(
            "grouped match structured dependency question iterable function root references should exist",
        );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true).expect(
            "grouped match structured dependency question iterable function root references with declaration should exist",
        );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "kids", 1),
                        nth_offset(source, "kids", 1) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 2),
                        nth_offset(source, "kids", 2) + "kids".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "kids", 3),
                        nth_offset(source, "kids", 3) + "kids".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_grouped_import_question_wrapped_dependency_static_iterable_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-static-iterable-root-queries-broken");
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

pub fn total() -> Int {
    for current in maybe_items? {
        let first = current.value
    }
    for current in maybe_items? {
        let second = current.value
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_items", 3));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question iterable static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question iterable static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question iterable static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("grouped dependency question iterable static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 2),
                        nth_offset(source, "maybe_items", 2) + "maybe_items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 3),
                        nth_offset(source, "maybe_items", 3) + "maybe_items".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect(
                "grouped dependency question iterable static root references with declaration should exist",
            );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
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
                        nth_offset(source, "maybe_items", 1),
                        nth_offset(source, "maybe_items", 1) + "maybe_items".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 2),
                        nth_offset(source, "maybe_items", 2) + "maybe_items".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_items", 3),
                        nth_offset(source, "maybe_items", 3) + "maybe_items".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_question_wrapped_dependency_function_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-function-root-queries");
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

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main() -> Int {
    return load_cfg()?.value + load_cfg()?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "load_cfg", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped dependency question function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("grouped dependency question function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 2),
                        nth_offset(source, "load_cfg", 2) + "load_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 3),
                        nth_offset(source, "load_cfg", 3) + "load_cfg".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("grouped dependency question function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 1),
                        nth_offset(source, "load_cfg", 1) + "load_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 2),
                        nth_offset(source, "load_cfg", 2) + "load_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 3),
                        nth_offset(source, "load_cfg", 3) + "load_cfg".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_grouped_import_question_wrapped_dependency_function_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-function-root-queries-broken");
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

pub struct Config {
    value: Int,
}

pub fn maybe_load() -> Option[Config]
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

use demo.dep.{maybe_load as load_cfg}

pub fn main() -> Int {
    let next = load_cfg()?.value + load_cfg()?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "load_cfg", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question function root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question function root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question function root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("grouped dependency question function root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 2),
                        nth_offset(source, "load_cfg", 2) + "load_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 3),
                        nth_offset(source, "load_cfg", 3) + "load_cfg".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("grouped dependency question function root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 1),
                        nth_offset(source, "load_cfg", 1) + "load_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 2),
                        nth_offset(source, "load_cfg", 2) + "load_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "load_cfg", 3),
                        nth_offset(source, "load_cfg", 3) + "load_cfg".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_grouped_import_question_wrapped_dependency_static_roots() {
    let temp = TempDir::new("ql-lsp-grouped-question-static-root-queries");
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

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main() -> Int {
    return maybe_cfg?.value + maybe_cfg?.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_cfg", 2));
    let hover = hover_for_package_analysis(source, &analysis, &package, root_position)
        .expect("grouped dependency question static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition =
        definition_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration =
        declaration_for_package_analysis(&uri, source, &analysis, &package, root_position)
            .expect("grouped dependency question static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, false)
            .expect("grouped dependency question static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 2),
                        nth_offset(source, "maybe_cfg", 2) + "maybe_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 3),
                        nth_offset(source, "maybe_cfg", 3) + "maybe_cfg".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_package_analysis(&uri, source, &analysis, &package, root_position, true)
            .expect("grouped dependency question static root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 1),
                        nth_offset(source, "maybe_cfg", 1) + "maybe_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 2),
                        nth_offset(source, "maybe_cfg", 2) + "maybe_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 3),
                        nth_offset(source, "maybe_cfg", 3) + "maybe_cfg".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_fallback_surfaces_grouped_import_question_wrapped_dependency_static_roots_without_semantic_analysis(
) {
    let temp = TempDir::new("ql-lsp-grouped-question-static-root-queries-broken");
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

pub struct Config {
    value: Int,
}

pub static MAYBE: Option[Config]
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

use demo.dep.{MAYBE as maybe_cfg}

pub fn main() -> Int {
    let next = maybe_cfg?.value + maybe_cfg?.value
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(source, nth_offset(source, "maybe_cfg", 2));
    let hover = hover_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question static root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition = definition_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question static root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let declaration = declaration_for_dependency_values(source, &package, root_position)
        .expect("grouped dependency question static root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(
        &location,
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );

    let without_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, false)
            .expect("grouped dependency question static root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 2),
                        nth_offset(source, "maybe_cfg", 2) + "maybe_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 3),
                        nth_offset(source, "maybe_cfg", 3) + "maybe_cfg".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration =
        references_for_dependency_values(&uri, source, &package, root_position, true)
            .expect("grouped dependency question static root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(
        &with_declaration[0],
        &dep_qi,
        "pub struct Config {\n    value: Int,\n}",
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 1),
                        nth_offset(source, "maybe_cfg", 1) + "maybe_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 2),
                        nth_offset(source, "maybe_cfg", 2) + "maybe_cfg".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "maybe_cfg", 3),
                        nth_offset(source, "maybe_cfg", 3) + "maybe_cfg".len(),
                    ),
                ),
            ),
        ]
    );
}
