mod common;

use common::request::{TempDir, formatting_via_request, initialized_service_with_open_documents};
use tower_lsp::lsp_types::{Position, Range, TextEdit, Url};

#[tokio::test(flavor = "current_thread")]
async fn formatting_request_returns_full_document_edit_when_qfmt_changes_source() {
    let temp = TempDir::new("ql-lsp-formatting-request-changed");
    let source = "fn main()->Int{return 1}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = formatting_via_request(&mut service, uri)
        .await
        .expect("formatting request should return edits for parseable source");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(0, 0), Position::new(1, 0)),
            "fn main() -> Int {\n    return 1\n}\n".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn formatting_request_returns_empty_edits_when_source_is_already_formatted() {
    let temp = TempDir::new("ql-lsp-formatting-request-unchanged");
    let source = "fn main() -> Int {\n    return 1\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = formatting_via_request(&mut service, uri)
        .await
        .expect("formatting request should return an empty edit list for formatted source");

    assert!(edits.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn formatting_request_returns_none_when_source_has_parse_errors() {
    let temp = TempDir::new("ql-lsp-formatting-request-parse-error");
    let source = "fn main( {\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = formatting_via_request(&mut service, uri).await;

    assert_eq!(edits, None);
}
