mod common;

use common::request::{
    TempDir, did_open_via_request, initialize_service_with_workspace_roots, offset_to_position,
    workspace_symbol_via_request,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::Url;

#[tokio::test(flavor = "current_thread")]
async fn workspace_symbol_request_uses_workspace_root_without_open_documents() {
    let temp = TempDir::new("ql-lsp-workspace-symbol-request-roots");
    let workspace_root = temp.path().join("workspace");
    let helper_path = temp.write(
        "workspace/packages/tool/src/helper.ql",
        r#"
package demo.tool

pub fn helper() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 0
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;

    let symbols = workspace_symbol_via_request(&mut service, "helper").await;

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "helper");
    assert_eq!(
        symbols[0]
            .location
            .uri
            .to_file_path()
            .expect("workspace symbol path should convert")
            .canonicalize()
            .expect("workspace symbol path should canonicalize"),
        helper_path
            .canonicalize()
            .expect("helper path should canonicalize"),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn workspace_symbol_request_prefers_open_unsaved_dependency_source() {
    let temp = TempDir::new("ql-lsp-workspace-symbol-request-open-doc");
    let workspace_root = temp.path().join("workspace");
    let dependency_source_path = temp.write(
        "workspace/vendor/dep/src/lib.ql",
        r#"
package demo.dep

pub fn disk_helper() -> Int {
    return 1
}
"#,
    );
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
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.dep.disk_helper as run

pub fn main() -> Int {
    return run(1)
}
"#,
    );
    temp.write(
        "workspace/vendor/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/vendor/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn disk_helper() -> Int
"#,
    );

    let open_source = r#"
package demo.dep

pub fn fresh_helper() -> Int {
    return 2
}
"#
    .to_owned();
    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let dependency_source_uri = Url::from_file_path(&dependency_source_path)
        .expect("dependency source path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(
        &mut service,
        dependency_source_uri.clone(),
        open_source.clone(),
    )
    .await;

    let symbols = workspace_symbol_via_request(&mut service, "fresh_helper").await;

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "fresh_helper");
    assert_eq!(symbols[0].location.uri, dependency_source_uri);
    let found_path = symbols[0]
        .location
        .uri
        .to_file_path()
        .expect("workspace symbol path should convert");
    assert_eq!(
        found_path
            .canonicalize()
            .expect("dependency source path should canonicalize"),
        dependency_source_path
            .canonicalize()
            .expect("dependency source path should canonicalize"),
    );
    assert_eq!(
        symbols[0].location.range.start,
        offset_to_position(
            &open_source,
            open_source
                .find("fresh_helper")
                .expect("fresh helper should exist")
        ),
        "workspace symbol should use the open document source",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn workspace_symbol_request_keeps_same_named_dependency_symbols_isolated_by_package() {
    let temp = TempDir::new("ql-lsp-workspace-symbol-request-same-name");
    let workspace_root = temp.path().join("workspace");
    let dependency_interface_path = temp.write(
        "workspace/vendor/dep-interface/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub fn beta() -> Int
"#,
    );
    temp.write(
        "workspace/vendor/dep-source/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/vendor/dep-source/src/lib.ql",
        r#"
package demo.dep.source

pub fn alpha() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/vendor/dep-source/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub fn alpha() -> Int
"#,
    );
    temp.write(
        "workspace/vendor/dep-interface/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
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

[references]
packages = ["../../vendor/dep-source", "../../vendor/dep-interface"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
pub fn main() -> Int {
    return 0
}
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;

    let symbols = workspace_symbol_via_request(&mut service, "beta").await;

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "beta");
    assert_eq!(
        symbols[0]
            .location
            .uri
            .to_file_path()
            .expect("workspace symbol path should convert")
            .canonicalize()
            .expect("workspace symbol path should canonicalize"),
        dependency_interface_path
            .canonicalize()
            .expect("dependency interface path should canonicalize"),
    );
}
