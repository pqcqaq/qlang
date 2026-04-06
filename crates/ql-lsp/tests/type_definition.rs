use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    span_to_range, type_definition_for_analysis, type_definition_for_dependency_imports,
    type_definition_for_package_analysis,
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

#[test]
fn type_definition_bridge_follows_same_file_explicit_type_uses() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
package demo.app

type UserId = Int

struct BoxedInt {
    value: Int,
}

fn identity[ValueType](value: ValueType) -> ValueType {
    let copy: ValueType = value
    copy
}

fn rename(user: UserId) -> UserId {
    let next: UserId = user
    next
}

fn make_box(value: Int) -> BoxedInt {
    BoxedInt { value: value }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let generic = type_definition_for_analysis(
        &uri,
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "ValueType", 4)),
    )
    .expect("generic type definition should exist");
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: generic_uri,
        range: generic_range,
    }) = generic
    else {
        panic!("type definition should be one location")
    };
    assert_eq!(generic_uri, uri);
    let generic_def = nth_offset(source, "ValueType", 1);
    assert_eq!(
        generic_range,
        span_to_range(
            source,
            ql_span::Span::new(generic_def, generic_def + "ValueType".len())
        )
    );

    let alias = type_definition_for_analysis(
        &uri,
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "UserId", 3)),
    )
    .expect("type alias definition should exist");
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: alias_uri,
        range: alias_range,
    }) = alias
    else {
        panic!("type definition should be one location")
    };
    assert_eq!(alias_uri, uri);
    let alias_def = nth_offset(source, "UserId", 1);
    assert_eq!(
        alias_range,
        span_to_range(
            source,
            ql_span::Span::new(alias_def, alias_def + "UserId".len())
        )
    );

    let struct_ty = type_definition_for_analysis(
        &uri,
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "BoxedInt", 2)),
    )
    .expect("struct type definition should exist");
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: struct_uri,
        range: struct_range,
    }) = struct_ty
    else {
        panic!("type definition should be one location")
    };
    assert_eq!(struct_uri, uri);
    let struct_def = nth_offset(source, "BoxedInt", 1);
    assert_eq!(
        struct_range,
        span_to_range(
            source,
            ql_span::Span::new(struct_def, struct_def + "BoxedInt".len())
        )
    );
}

#[test]
fn type_definition_bridge_prefers_underlying_local_type_for_import_alias_uses() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
package demo.app

use UserId as Handle

type UserId = Int

fn copy(value: Handle) -> Handle {
    let local: Handle = value
    local
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let definition = type_definition_for_analysis(
        &uri,
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "Handle", 4)),
    )
    .expect("import-alias type definition should exist");
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("type definition should be one location")
    };
    assert_eq!(definition_uri, uri);
    let alias_def = nth_offset(source, "UserId", 2);
    assert_eq!(
        range,
        span_to_range(
            source,
            ql_span::Span::new(alias_def, alias_def + "UserId".len())
        )
    );
}

#[test]
fn type_definition_bridge_follows_dependency_type_roots() {
    let temp = TempDir::new("ql-lsp-type-definition");
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

pub struct Buffer[T] {
    value: T,
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

use demo.dep.Buffer as Buf

pub fn main(value: Buf[Int]) -> Buf[Int] {
    value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let definition = type_definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Buf", 3)),
    )
    .expect("dependency type definition should exist");
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

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub struct Buffer[T] {\n    value: T,\n}";
    let struct_def = artifact
        .find(snippet)
        .expect("struct signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(
            &artifact,
            ql_span::Span::new(struct_def, struct_def + snippet.len())
        )
    );
}

#[test]
fn type_definition_bridge_follows_dependency_type_roots_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-type-definition-broken");
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

pub struct Buffer[T] {
    value: T,
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

use demo.dep.Buffer as Buf

pub fn main(value: Buf[Int]) -> Int {
    let next = missing(value)
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let definition = type_definition_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Buf", 3)),
    )
    .expect("dependency type definition should exist even without semantic analysis");
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

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub struct Buffer[T] {\n    value: T,\n}";
    let struct_def = artifact
        .find(snippet)
        .expect("struct signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(
            &artifact,
            ql_span::Span::new(struct_def, struct_def + snippet.len())
        )
    );
}
