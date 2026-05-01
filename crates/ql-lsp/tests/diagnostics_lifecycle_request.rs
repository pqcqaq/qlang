mod common;

use common::request::{
    TempDir, did_change_via_request, did_close_via_request, did_open_via_request,
    initialize_service,
};
use futures_util::{FutureExt, StreamExt};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{PublishDiagnosticsParams, Url};

fn next_publish_diagnostics(socket: &mut tower_lsp::ClientSocket) -> PublishDiagnosticsParams {
    loop {
        let request = socket
            .next()
            .now_or_never()
            .flatten()
            .expect("server should publish diagnostics synchronously");
        if request.method() == "textDocument/publishDiagnostics" {
            return serde_json::from_value(
                request
                    .params()
                    .cloned()
                    .expect("publishDiagnostics should include params"),
            )
            .expect("publishDiagnostics params should deserialize");
        }
    }
}

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
