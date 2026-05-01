mod common;

use common::request::{
    TempDir, folding_range_via_request, initialized_service_with_open_documents,
    inlay_hint_via_request, nth_offset, offset_to_position, selection_range_via_request,
    signature_help_via_request,
};
use tower_lsp::lsp_types::{InlayHintLabel, Range, Url};

#[tokio::test(flavor = "current_thread")]
async fn rich_editor_requests_cover_signature_inlay_folding_and_selection() {
    let temp = TempDir::new("ql-lsp-rich-editor-requests");
    let source_path = temp.write(
        "rich.ql",
        r#"
fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    let total = add(1, 2)
    if total > 0 {
        return total
    }
    return 0
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    let signature = signature_help_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "add(1, ", 1) + "add(1, ".len()),
    )
    .await
    .expect("signatureHelp should return callable signature");
    assert_eq!(signature.active_parameter, Some(1));
    assert_eq!(
        signature.signatures[0].label,
        "fn add(left: Int, right: Int) -> Int"
    );

    let full_range = Range::new(
        offset_to_position(&source, 0),
        offset_to_position(&source, source.len()),
    );
    let hints = inlay_hint_via_request(&mut service, uri.clone(), full_range)
        .await
        .expect("inlayHint should return inferred local hints");
    assert!(
        hints
            .iter()
            .any(|hint| matches!(&hint.label, InlayHintLabel::String(label) if label == ": Int")),
        "inlay hints should include inferred Int local type: {hints:#?}",
    );

    let folds = folding_range_via_request(&mut service, uri.clone())
        .await
        .expect("foldingRange should return block ranges");
    assert!(
        folds.iter().any(|range| range.start_line < range.end_line),
        "foldingRange should include multiline blocks: {folds:#?}",
    );

    let selections = selection_range_via_request(
        &mut service,
        uri,
        vec![offset_to_position(&source, nth_offset(&source, "total", 2))],
    )
    .await
    .expect("selectionRange should return token selection");
    assert_eq!(selections.len(), 1);
    assert!(
        selections[0].parent.is_some(),
        "selectionRange should include parent expansion: {selections:#?}",
    );
}
