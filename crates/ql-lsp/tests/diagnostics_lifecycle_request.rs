mod common;

use common::request::{
    TempDir, did_change_via_request, did_close_via_request, did_open_via_request,
    initialize_service, initialize_service_with_workspace_roots, next_publish_diagnostics,
};
use common::stdlib_real::write_real_stdlib_workspace;
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Url};

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_notifications_follow_open_change_and_close_lifecycle() {
    let temp = TempDir::new("ql-lsp-diagnostics-lifecycle-request");
    let source_path = temp.write(
        "sample.ql",
        r#"
fn main() -> Int {
    return 1
}
"#,
    );
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let (mut service, mut socket) = LspService::new(Backend::new);
    initialize_service(&mut service).await;

    did_open_via_request(
        &mut service,
        uri.clone(),
        std::fs::read_to_string(&source_path).expect("source should read"),
    )
    .await;
    let open_diagnostics = next_publish_diagnostics(&mut socket);
    assert_eq!(open_diagnostics.uri, uri);
    assert!(
        open_diagnostics.diagnostics.is_empty(),
        "valid source should publish an empty diagnostics list"
    );

    did_change_via_request(&mut service, uri.clone(), 2, "fn main( {\n".to_owned()).await;
    let change_diagnostics = next_publish_diagnostics(&mut socket);
    assert_eq!(change_diagnostics.uri, uri);
    assert!(
        change_diagnostics
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expected parameter name")),
        "parse-error source should publish parser diagnostics: {change_diagnostics:#?}",
    );

    did_close_via_request(&mut service, uri.clone()).await;
    let close_diagnostics = next_publish_diagnostics(&mut socket);
    assert_eq!(close_diagnostics.uri, uri);
    assert!(
        close_diagnostics.diagnostics.is_empty(),
        "closing the document should clear diagnostics"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_notifications_include_package_interface_errors_for_clean_sources() {
    let temp = TempDir::new("ql-lsp-diagnostics-package-interface-request");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 1
}
"#,
    );

    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let (mut service, mut socket) = LspService::new(Backend::new);
    initialize_service(&mut service).await;

    did_open_via_request(&mut service, uri.clone(), source).await;
    let open_diagnostics = next_publish_diagnostics(&mut socket);
    assert_eq!(open_diagnostics.uri, uri);
    assert_eq!(
        open_diagnostics.diagnostics.len(),
        1,
        "clean package source should publish package preflight diagnostics: {open_diagnostics:#?}",
    );
    let diagnostic = &open_diagnostics.diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "package-interface-not-found".to_owned()
        ))
    );
    assert!(
        diagnostic
            .message
            .contains("referenced package `dep` is missing interface artifact"),
        "missing interface package diagnostic should be reported: {diagnostic:#?}",
    );

    did_change_via_request(&mut service, uri.clone(), 2, "fn main( {\n".to_owned()).await;
    let change_diagnostics = next_publish_diagnostics(&mut socket);
    assert_eq!(change_diagnostics.uri, uri);
    assert!(
        change_diagnostics
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expected parameter name")),
        "parse-error buffer should keep parser diagnostics: {change_diagnostics:#?}",
    );
    assert!(
        change_diagnostics
            .diagnostics
            .iter()
            .all(|diagnostic| { !diagnostic.message.contains("missing interface artifact") }),
        "package preflight diagnostics should not mask current buffer parser diagnostics",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_notifications_accept_current_real_stdlib_workspace() {
    let temp = TempDir::new("ql-lsp-diagnostics-real-stdlib-workspace");
    let app_source = r#"
package demo.app

use std.option.Option as Option
use std.option.some as option_some
use std.result.ok_or as result_ok_or
use std.array.repeat_array as repeat_array
use std.core.clamp_int as clamp_int
use std.test.expect_eq as expect_eq

pub fn main() -> Int {
    let option_value: Option[Int] = option_some(42)
    let result_value = result_ok_or(option_value, 5)
    let values: [Int; 3] = repeat_array(2)
    return expect_eq(clamp_int(42, 0, 100), 42)
}
"#;
    let workspace = write_real_stdlib_workspace(&temp, app_source);
    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root should convert to URI");
    let (mut service, mut socket) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;

    did_open_via_request(
        &mut service,
        workspace.app_uri.clone(),
        app_source.to_owned(),
    )
    .await;
    let diagnostics = next_publish_diagnostics(&mut socket);
    assert_eq!(diagnostics.uri, workspace.app_uri);
    assert!(
        diagnostics.diagnostics.is_empty(),
        "current real stdlib API should not publish diagnostics: {diagnostics:#?}",
    );
}
