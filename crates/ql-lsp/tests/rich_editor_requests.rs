mod common;

use common::request::{
    TempDir, folding_range_via_request, initialized_service_with_open_documents,
    inlay_hint_via_request, nth_offset, offset_to_position, selection_range_via_request,
    signature_help_via_request,
};
use tower_lsp::lsp_types::{FoldingRangeKind, InlayHintKind, InlayHintLabel, Range, Url};

#[tokio::test(flavor = "current_thread")]
async fn rich_editor_requests_cover_signature_inlay_folding_and_selection() {
    let temp = TempDir::new("ql-lsp-rich-editor-requests");
    let source_path = temp.write(
        "rich.ql",
        r#"
/* module fold
   stays foldable
*/
fn add(left: Int, right: Int) -> Int {
    return left + right
}

// group fold alpha
// group fold beta
struct Counter { value: Int }

impl Counter {
    fn add(self, delta: Int, scale: Int) -> Int {
        return self.value + delta * scale
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    let total = add(1, 2)
    let next = counter.add(3, 4)
    let marker: String = "not a comment
// not a line comment
/* not a block comment */
// still not a line comment"
    if total > 0 {
        return next
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
    assert!(
        hints.iter().any(
            |hint| matches!((&hint.kind, &hint.label), (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == "left:")
        ),
        "inlay hints should include function parameter name `left`: {hints:#?}",
    );
    assert!(
        hints.iter().any(
            |hint| matches!((&hint.kind, &hint.label), (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == "right:")
        ),
        "inlay hints should include function parameter name `right`: {hints:#?}",
    );
    assert!(
        hints.iter().any(
            |hint| matches!((&hint.kind, &hint.label), (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == "delta:")
        ),
        "inlay hints should skip method `self` and include `delta`: {hints:#?}",
    );
    assert!(
        hints.iter().any(
            |hint| matches!((&hint.kind, &hint.label), (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == "scale:")
        ),
        "inlay hints should skip method `self` and include `scale`: {hints:#?}",
    );

    let folds = folding_range_via_request(&mut service, uri.clone())
        .await
        .expect("foldingRange should return block ranges");
    assert!(
        folds.iter().any(|range| range.start_line < range.end_line),
        "foldingRange should include multiline blocks: {folds:#?}",
    );
    let comment_folds = folds
        .iter()
        .filter(|range| range.kind.as_ref() == Some(&FoldingRangeKind::Comment))
        .collect::<Vec<_>>();
    assert_eq!(
        comment_folds.len(),
        2,
        "foldingRange should include only real comment folds, not string markers: {folds:#?}",
    );
    let block_comment_line =
        offset_to_position(&source, nth_offset(&source, "/* module fold", 1)).line;
    let line_comment_line =
        offset_to_position(&source, nth_offset(&source, "// group fold alpha", 1)).line;
    assert!(
        comment_folds
            .iter()
            .any(|range| range.start_line == block_comment_line),
        "foldingRange should include block comment folds: {folds:#?}",
    );
    assert!(
        comment_folds
            .iter()
            .any(|range| range.start_line == line_comment_line),
        "foldingRange should include consecutive line comment folds: {folds:#?}",
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
