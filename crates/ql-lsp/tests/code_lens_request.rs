mod common;

use common::request::{
    TempDir, code_lens_resolve_via_request, code_lens_via_request, did_open_via_request,
    initialize_service_with_workspace_roots, initialized_service_with_open_documents, nth_offset,
    offset_to_position,
};
use common::stdlib_real::{real_stdlib_source_path, write_real_stdlib_workspace};
use ql_lsp::Backend;
use std::fs;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{CodeLens, Location, Position, Url};

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_returns_references_and_implementation_lenses() {
    let temp = TempDir::new("ql-lsp-code-lens-request");
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

fn helper() -> Int {
    return 2
}

fn main(worker: Worker) -> Int {
    return helper() + worker.run()
}
"#;
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let lenses = code_lens_via_request(&mut service, uri)
        .await
        .expect("codeLens request should return lenses");

    let helper_position = offset_to_position(source, nth_offset(source, "helper", 1));
    let helper_lens = reference_lens_at(&lenses, helper_position)
        .unwrap_or_else(|| panic!("helper reference code lens should exist: {lenses:#?}"));
    assert_eq!(
        helper_lens
            .command
            .as_ref()
            .and_then(|command| command.arguments.as_ref())
            .map(Vec::len),
        Some(3),
    );

    let runner_position = offset_to_position(source, nth_offset(source, "Runner", 1));
    assert!(
        lenses.iter().any(|lens| {
            lens.range.start == runner_position
                && lens.command.as_ref().is_some_and(|command| {
                    command.title == "1 implementation"
                        && command.command == "editor.action.showReferences"
                })
        }),
        "trait implementation code lens should exist: {lenses:#?}",
    );

    let resolved = code_lens_resolve_via_request(&mut service, helper_lens.clone()).await;
    assert_eq!(resolved, *helper_lens);
}

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_counts_workspace_dependency_consumers() {
    let temp = TempDir::new("ql-lsp-code-lens-request-workspace");
    let workspace_root = temp.path().join("workspace");
    let core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 1
}
"#;
    let app_source = r#"
package demo.app

use demo.core.helper as helper

pub fn main() -> Int {
    return helper()
}
"#;
    let core_path = temp.write("workspace/packages/core/src/lib.ql", core_source);
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
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

[dependencies]
core = { path = "../core" }
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.to_owned()).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let lenses = code_lens_via_request(&mut service, core_uri.clone())
        .await
        .expect("workspace codeLens request should return dependency source lenses");

    let helper_position = offset_to_position(core_source, nth_offset(core_source, "helper", 1));
    let helper_lens = reference_lens_at(&lenses, helper_position).unwrap_or_else(|| {
        panic!("workspace dependency helper reference code lens should exist: {lenses:#?}")
    });
    let locations = helper_lens
        .command
        .as_ref()
        .and_then(|command| command.arguments.as_ref())
        .and_then(|arguments| arguments.get(2).cloned())
        .and_then(|value| serde_json::from_value::<Vec<Location>>(value).ok())
        .expect("codeLens command should carry reference locations");
    assert_eq!(locations.len(), 1);
    assert!(
        locations.iter().all(|location| location.uri == app_uri),
        "workspace dependency codeLens should point at app consumers only: {locations:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_counts_open_workspace_consumers_without_dependency_interface() {
    let temp = TempDir::new("ql-lsp-code-lens-request-open-consumer-missing-interface");
    let workspace_root = temp.path().join("workspace");
    let dep_source = r#"
package demo.dep

pub fn helper() -> Int {
    return 1
}
"#;
    let app_source = r#"
package demo.app

use demo.dep.helper as helper

pub fn main() -> Int {
    return helper()
}
"#;
    let dep_path = temp.write("workspace/vendor/dep/src/lib.ql", dep_source);
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = { path = "../../vendor/dep" }
"#,
    );
    temp.write(
        "workspace/vendor/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let dep_uri = Url::from_file_path(&dep_path).expect("dependency path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, dep_uri.clone(), dep_source.to_owned()).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let lenses = code_lens_via_request(&mut service, dep_uri)
        .await
        .expect("workspace codeLens request should return dependency source lenses");

    let helper_position = offset_to_position(dep_source, nth_offset(dep_source, "helper", 1));
    let helper_lens = reference_lens_at(&lenses, helper_position).unwrap_or_else(|| {
        panic!("workspace dependency helper reference code lens should exist: {lenses:#?}")
    });
    let locations = helper_lens
        .command
        .as_ref()
        .and_then(|command| command.arguments.as_ref())
        .and_then(|arguments| arguments.get(2).cloned())
        .and_then(|value| serde_json::from_value::<Vec<Location>>(value).ok())
        .expect("codeLens command should carry reference locations");
    assert!(
        locations.iter().any(|location| location.uri == app_uri),
        "workspace dependency codeLens should include the open app consumer even without an interface artifact: {locations:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_counts_real_stdlib_consumers() {
    let temp = TempDir::new("ql-lsp-code-lens-real-stdlib");
    let app_source = r#"
package demo.app

use std.core.max_int as max_int

pub fn main() -> Int {
    return max_int(1, 2)
}
"#;
    let workspace = write_real_stdlib_workspace(&temp, app_source);
    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let core_path = real_stdlib_source_path(&workspace.stdlib_root, "core");
    let core_source = fs::read_to_string(&core_path)
        .expect("real std.core temp source should read")
        .replace("\r\n", "\n");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(
        &mut service,
        workspace.app_uri.clone(),
        app_source.to_owned(),
    )
    .await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;

    let lenses = code_lens_via_request(&mut service, core_uri)
        .await
        .expect("real stdlib codeLens request should return source lenses");

    let max_int_position = offset_to_position(&core_source, nth_offset(&core_source, "max_int", 1));
    let max_int_lens = reference_lens_at(&lenses, max_int_position).unwrap_or_else(|| {
        panic!("std.core max_int reference code lens should exist: {lenses:#?}")
    });
    let locations = max_int_lens
        .command
        .as_ref()
        .and_then(|command| command.arguments.as_ref())
        .and_then(|arguments| arguments.get(2).cloned())
        .and_then(|value| serde_json::from_value::<Vec<Location>>(value).ok())
        .expect("real stdlib codeLens command should carry reference locations");
    assert!(
        locations
            .iter()
            .any(|location| location.uri == workspace.app_uri),
        "real stdlib codeLens should include app consumer references: {locations:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_returns_none_for_parse_errors() {
    let temp = TempDir::new("ql-lsp-code-lens-request-parse-error");
    let source = "fn broken( -> Int {\n    return 1\n}\n";
    let source_path = temp.write("broken.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let lenses = code_lens_via_request(&mut service, uri).await;

    assert_eq!(lenses, None);
}

fn reference_lens_at(lenses: &[CodeLens], position: Position) -> Option<&CodeLens> {
    lenses.iter().find(|lens| {
        lens.range.start == position
            && lens.command.as_ref().is_some_and(|command| {
                (command.title.ends_with("reference") || command.title.ends_with("references"))
                    && command.command == "editor.action.showReferences"
            })
    })
}
