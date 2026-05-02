use super::*;

#[test]
fn implementation_for_analysis_returns_scalar_for_trait_implementations() {
    let source = r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}
"#;
    let analysis = analyze_source(source).expect("analysis should succeed");
    let uri = Url::parse("file:///test.ql").expect("uri should parse");

    let implementation = implementation_for_analysis(
        &uri,
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "Runner", 1)),
    )
    .expect("trait implementation should exist");

    let GotoDefinitionResponse::Scalar(location) = implementation else {
        panic!("single trait implementation should resolve to one location")
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(
            source,
            Span::new(
                nth_offset(source, "impl Runner for Worker", 1),
                source.rfind('}').expect("impl block should close") + 1,
            ),
        )
    );
}

#[test]
fn implementation_for_analysis_returns_array_for_trait_method_implementations() {
    let source = r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}
struct Helper {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}

impl Runner for Helper {
    fn run(self) -> Int {
        return 2
    }
}
"#;
    let analysis = analyze_source(source).expect("analysis should succeed");
    let uri = Url::parse("file:///test.ql").expect("uri should parse");

    let implementation = implementation_for_analysis(
        &uri,
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "run", 1)),
    )
    .expect("trait method implementations should exist");

    let GotoDefinitionResponse::Array(locations) = implementation else {
        panic!("multiple trait method implementations should resolve to many locations")
    };
    assert_eq!(
        locations,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "run", 2))
            ),
            Location::new(uri, span_to_range(source, nth_span(source, "run", 3))),
        ]
    );
}

#[test]
fn implementation_for_analysis_returns_none_on_method_definition_site() {
    let source = r#"
struct Config {
    value: Int,
}

impl Config {
    fn get(self) -> Int {
        return self.value
    }
}

fn read(config: Config) -> Int {
    return config.get()
}
"#;
    let analysis = analyze_source(source).expect("analysis should succeed");
    let uri = Url::parse("file:///test.ql").expect("uri should parse");

    assert_eq!(
        implementation_for_analysis(
            &uri,
            source,
            &analysis,
            offset_to_position(source, nth_offset(source, "get", 1)),
        ),
        None,
    );
}

#[test]
fn workspace_type_import_implementation_prefers_workspace_member_source_over_interface_artifact() {
    let temp = TempDir::new("ql-lsp-workspace-type-import-implementation");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
    );
    let core_source_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
    );

    let source = fs::read_to_string(&app_path).expect("app source should read");
    let analysis = analyze_source(&source).expect("app source should analyze");
    let package = package_analysis_for_path(&app_path).expect("package analysis should succeed");

    let implementation = workspace_source_implementation_for_dependency_with_open_docs(
        &source,
        Some(&analysis),
        &package,
        &file_open_documents(vec![]),
        offset_to_position(&source, nth_offset(&source, "Config", 2)),
    )
    .expect("workspace import implementation should exist");

    let GotoDefinitionResponse::Array(locations) = implementation else {
        panic!("workspace import implementation should resolve to many locations")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations.iter().all(|location| {
            location
                .uri
                .to_file_path()
                .ok()
                .and_then(|path| fs::canonicalize(path).ok())
                == Some(
                    fs::canonicalize(&core_source_path)
                        .expect("core source path should canonicalize"),
                )
        }),
        "all implementation locations should point at workspace source",
    );
    assert_eq!(
        locations[0].range.start,
        offset_to_position(
            &fs::read_to_string(&core_source_path)
                .expect("core source should read")
                .replace("\r\n", "\n"),
            nth_offset(
                &fs::read_to_string(&core_source_path)
                    .expect("core source should read")
                    .replace("\r\n", "\n"),
                "impl Config",
                1
            )
        ),
    );
    assert_eq!(
        locations[1].range.start,
        offset_to_position(
            &fs::read_to_string(&core_source_path)
                .expect("core source should read")
                .replace("\r\n", "\n"),
            nth_offset(
                &fs::read_to_string(&core_source_path)
                    .expect("core source should read")
                    .replace("\r\n", "\n"),
                "extend Config",
                1
            )
        ),
    );
}

#[test]
fn workspace_dependency_non_import_positions_implementation_prefer_workspace_member_source_over_interface_artifact()
 {
    let temp = TempDir::new("ql-lsp-workspace-dependency-source-implementation-positions");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.Holder as Hold

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let holder = Hold { child: config.clone_self() }
    let command = Cmd.Retry(1)
    return holder.child.value + built.value + command.unwrap_or(0)
}
"#,
    );
    let core_source_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

impl Config {
    pub fn clone_self(self) -> Config {
        return self
    }
}

extend Config {
    pub fn extra(self) -> Int {
        return self.limit
    }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

impl Config {
    pub fn clone_self(self) -> Config
}
"#,
    );

    let source = fs::read_to_string(&app_path).expect("app source should read");
    let analysis = analyze_source(&source).expect("app source should analyze");
    let package = package_analysis_for_path(&app_path).expect("package analysis should succeed");
    let core_source = fs::read_to_string(&core_source_path)
        .expect("core source should read")
        .replace("\r\n", "\n");
    let expected_path =
        fs::canonicalize(&core_source_path).expect("core source path should canonicalize");

    for (needle, occurrence, expected_markers) in [
        ("built", 2usize, &["impl Config", "extend Config"][..]),
        ("Retry", 1usize, &["impl Command"][..]),
        ("child", 2usize, &["impl Config", "extend Config"][..]),
    ] {
        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &source,
            Some(&analysis),
            &package,
            &file_open_documents(vec![]),
            offset_to_position(&source, nth_offset(&source, needle, occurrence)),
        )
        .unwrap_or_else(|| panic!("workspace dependency implementation should exist for {needle}"));

        let locations = match implementation {
            GotoDefinitionResponse::Scalar(location) => vec![location],
            GotoDefinitionResponse::Array(locations) => locations,
            GotoDefinitionResponse::Link(_) => {
                panic!("workspace dependency implementation should resolve to locations")
            }
        };
        assert_eq!(
            locations.len(),
            expected_markers.len(),
            "workspace dependency implementation should return all source impl blocks for {needle}",
        );
        for location in &locations {
            assert_eq!(
                location
                    .uri
                    .to_file_path()
                    .expect("implementation URI should convert to a file path")
                    .canonicalize()
                    .expect("implementation path should canonicalize"),
                expected_path,
            );
        }
        for marker in expected_markers {
            assert!(
                locations.iter().any(|location| {
                    location.range.start
                        == offset_to_position(&core_source, nth_offset(&core_source, marker, 1))
                }),
                "workspace dependency implementation should include {marker} for {needle}",
            );
        }
    }
}
