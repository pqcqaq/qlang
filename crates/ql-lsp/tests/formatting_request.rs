mod common;

use common::request::{
    TempDir, formatting_via_request, initialized_service_with_open_documents,
    on_type_formatting_via_request, range_formatting_via_request,
};
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

#[tokio::test(flavor = "current_thread")]
async fn range_formatting_request_returns_safe_local_edit() {
    let temp = TempDir::new("ql-lsp-range-formatting-request-local");
    let source = "fn main() -> Int {\nreturn 1\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = range_formatting_via_request(
        &mut service,
        uri,
        Range::new(
            Position::new(1, 0),
            Position::new(1, "return 1".len() as u32),
        ),
    )
    .await
    .expect("rangeFormatting should return edits for parseable source");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(1, 0), Position::new(1, 0)),
            "    ".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn range_formatting_request_returns_only_edits_inside_requested_range() {
    let temp = TempDir::new("ql-lsp-range-formatting-request-inside");
    let source = "fn main() -> Int {\nreturn 1\nreturn 2\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = range_formatting_via_request(
        &mut service,
        uri,
        Range::new(
            Position::new(2, 0),
            Position::new(2, "return 2".len() as u32),
        ),
    )
    .await
    .expect("rangeFormatting should return safe edits inside the requested range");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(2, 0), Position::new(2, 0)),
            "    ".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn on_type_formatting_request_formats_near_trigger() {
    let temp = TempDir::new("ql-lsp-on-type-formatting-request");
    let source = "fn main() -> Int {\nreturn 1\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = on_type_formatting_via_request(&mut service, uri, Position::new(1, 0), "\n")
        .await
        .expect("onTypeFormatting should return edits for parseable source");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(1, 0), Position::new(1, 0)),
            "    ".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn on_type_formatting_request_only_formats_trigger_line() {
    let temp = TempDir::new("ql-lsp-on-type-formatting-request-wide");
    let source = "fn main() -> Int {\nreturn 1\nlet x = 2\nlet y = 3\nreturn x + y\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = on_type_formatting_via_request(&mut service, uri, Position::new(4, 0), "\n")
        .await
        .expect("onTypeFormatting should return only trigger-line edits");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(4, 0), Position::new(4, 0)),
            "    ".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn on_type_formatting_request_formats_closing_brace_trigger() {
    let temp = TempDir::new("ql-lsp-on-type-formatting-request-brace");
    let source = "fn main() -> Int {\n    return 1\n    }\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = on_type_formatting_via_request(&mut service, uri, Position::new(2, 5), "}")
        .await
        .expect("onTypeFormatting should format the closing brace line");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(2, 0), Position::new(2, 4)),
            String::new(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn on_type_formatting_request_formats_semicolon_trigger() {
    let temp = TempDir::new("ql-lsp-on-type-formatting-request-semi");
    let source = "fn main() -> Int {\n1;\nreturn 2\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = on_type_formatting_via_request(&mut service, uri, Position::new(1, 2), ";")
        .await
        .expect("onTypeFormatting should format the semicolon line");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(1, 0), Position::new(1, 0)),
            "    ".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn on_type_formatting_request_formats_comma_trigger() {
    let temp = TempDir::new("ql-lsp-on-type-formatting-request-comma");
    let source = "struct Point {\nx: Int,\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = on_type_formatting_via_request(&mut service, uri, Position::new(1, 7), ",")
        .await
        .expect("onTypeFormatting should format the comma line");

    assert_eq!(
        edits,
        vec![TextEdit::new(
            Range::new(Position::new(1, 0), Position::new(1, 0)),
            "    ".to_owned(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn on_type_formatting_request_ignores_unadvertised_triggers() {
    let temp = TempDir::new("ql-lsp-on-type-formatting-request-unadvertised");
    let source = "fn main() -> Int {\nreturn 1\n}\n";
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let edits = on_type_formatting_via_request(&mut service, uri, Position::new(1, 0), "x")
        .await
        .expect("onTypeFormatting should return an empty edit list for ignored triggers");

    assert!(edits.is_empty());
}
