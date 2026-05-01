mod common;

use common::request::{
    TempDir, initialized_service_with_open_documents, nth_offset, offset_to_position,
    prepare_rename_via_request, rename_via_request,
};
use tower_lsp::lsp_types::{PrepareRenameResponse, Range, TextEdit, Url};

fn range_for(source: &str, needle: &str, occurrence: usize) -> Range {
    let start = nth_offset(source, needle, occurrence);
    Range::new(
        offset_to_position(source, start),
        offset_to_position(source, start + needle.len()),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn rename_request_returns_same_file_workspace_edits() {
    let temp = TempDir::new("ql-lsp-rename-request-same-file");
    let source_path = temp.write(
        "sample.ql",
        r#"
fn id(value: Int) -> Int {
    return value
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;
    let use_position = offset_to_position(&source, nth_offset(&source, "value", 2));

    let prepare = prepare_rename_via_request(&mut service, uri.clone(), use_position)
        .await
        .expect("prepareRename request should return a rename target");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("prepareRename request should return range plus placeholder")
    };
    assert_eq!(range, range_for(&source, "value", 2));
    assert_eq!(placeholder, "value");

    let edit = rename_via_request(&mut service, uri.clone(), use_position, "input")
        .await
        .expect("rename request should return workspace edits");
    let changes = edit
        .changes
        .expect("rename request should use simple document changes");
    let edits = changes
        .get(&uri)
        .expect("rename request should return edits for the open document");

    assert_eq!(
        edits,
        &vec![
            TextEdit::new(range_for(&source, "value", 1), "input".to_owned()),
            TextEdit::new(range_for(&source, "value", 2), "input".to_owned()),
        ]
    );
}
