use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies};
use ql_lsp::bridge::{
    declaration_for_dependency_values, definition_for_dependency_values,
    hover_for_dependency_values, references_for_dependency_values, span_to_range,
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

fn assert_targets_dependency_struct(location: Location, dep_qi: &Path, snippet: &str) {
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

    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .expect("struct declaration should exist in dependency artifact");
    assert_eq!(
        location.range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

fn app_uri(path: &Path) -> Url {
    Url::from_file_path(path).expect("app path should convert to file URL")
}

fn assert_dependency_location(location: &Location, dep_qi: &Path, snippet: &str) {
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
    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .expect("struct declaration should exist in dependency artifact");
    assert_eq!(
        location.range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

#[test]
fn broken_source_hover_falls_back_to_dependency_value_root() {
    let temp = TempDir::new("ql-lsp-value-root-hover");
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

pub struct Config {
    value: Int,
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
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config
    let broken: Int = "oops"
    return current.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let hover = hover_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "current", 2)),
    )
    .expect("dependency value hover should exist even without semantic analysis");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));
}

#[test]
fn broken_source_definition_falls_back_to_dependency_value_root() {
    let temp = TempDir::new("ql-lsp-value-root-definition");
    let app_root = temp.path().join("workspace").join("app");

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

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config
    let broken: Int = "oops"
    return current.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "current", 2)),
    )
    .expect("dependency value definition should exist even without semantic analysis");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_targets_dependency_struct(location, &dep_qi, "pub struct Config {\n    value: Int,\n}");
}

#[test]
fn broken_source_declaration_falls_back_to_dependency_self_receiver_root() {
    let temp = TempDir::new("ql-lsp-value-root-declaration-self");
    let app_root = temp.path().join("workspace").join("app");

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

use demo.dep.Config as Cfg

extend Cfg {
    pub fn read(self) -> Int {
        let broken: Int = "oops"
        return self.value
    }
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let declaration = declaration_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "self", 2)),
    )
    .expect("dependency value declaration should exist even without semantic analysis");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_targets_dependency_struct(location, &dep_qi, "pub struct Config {\n    value: Int,\n}");
}

#[test]
fn broken_source_references_fall_back_to_dependency_named_local_root() {
    let temp = TempDir::new("ql-lsp-value-root-references");
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

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config
    let alias = current
    let broken: Int = "oops"
    return current.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = app_uri(&app_path);
    let current_definition = nth_offset(source, "current", 1);

    let local_only = references_for_dependency_values(
        &uri,
        source,
        &package,
        offset_to_position(source, current_definition),
        false,
    )
    .expect("dependency value references should exist without semantic analysis");
    assert_eq!(
        local_only,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len()
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len()
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_dependency_values(
        &uri,
        source,
        &package,
        offset_to_position(source, current_definition),
        true,
    )
    .expect("dependency value references with declaration should exist");
    let snippet = "pub struct Config {\n    value: Int,\n}";
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(&with_declaration[0], &dep_qi, snippet);
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(current_definition, current_definition + "current".len()),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len()
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len()
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn broken_source_references_fall_back_to_dependency_self_receiver_root() {
    let temp = TempDir::new("ql-lsp-value-root-self-references");
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

use demo.dep.Config as Cfg

extend Cfg {
    pub fn read(self) -> Int {
        let alias = self
        let broken: Int = "oops"
        return self.value
    }
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = app_uri(&app_path);

    let references = references_for_dependency_values(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "self", 1)),
        true,
    )
    .expect("dependency self references should exist without semantic analysis");
    let snippet = "pub struct Config {\n    value: Int,\n}";
    assert_eq!(references.len(), 4);
    assert_dependency_location(&references[0], &dep_qi, snippet);
    assert_eq!(
        references[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "self", 1),
                        nth_offset(source, "self", 1) + "self".len()
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "self", 2),
                        nth_offset(source, "self", 2) + "self".len()
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "self", 3),
                        nth_offset(source, "self", 3) + "self".len()
                    ),
                ),
            ),
        ]
    );
}
