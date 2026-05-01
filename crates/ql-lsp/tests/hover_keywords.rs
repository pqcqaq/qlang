mod common;

use common::request::{
    TempDir, hover_via_request, initialized_service_with_open_documents, nth_offset,
    offset_to_position,
};
use tower_lsp::lsp_types::{HoverContents, Url};

fn hover_markup(hover: tower_lsp::lsp_types::Hover) -> String {
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("keyword hover should use markdown")
    };
    markup.value
}

#[tokio::test(flavor = "current_thread")]
async fn keyword_hover_documents_core_language_keywords() {
    let temp = TempDir::new("ql-lsp-keyword-hover");
    let source_path = temp.write(
        "keywords.ql",
        r#"
package app.demo

trait Show where T satisfies Show {
    fn show(self) -> String
}

async fn main() -> Int {
    let value = none
    if true {
        return await load()
    } else {
        loop {
            break
        }
    }
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    for keyword in [
        "package",
        "where",
        "satisfies",
        "fn",
        "self",
        "async",
        "none",
        "await",
        "loop",
    ] {
        let hover = hover_via_request(
            &mut service,
            uri.clone(),
            offset_to_position(&source, nth_offset(&source, keyword, 1)),
        )
        .await
        .unwrap_or_else(|| panic!("{keyword} should return keyword hover"));
        let markup = hover_markup(hover);
        assert!(
            markup.contains(&format!("keyword `{keyword}`")),
            "{keyword} hover should identify the keyword: {markup}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn escaped_keyword_identifier_does_not_use_keyword_hover() {
    let temp = TempDir::new("ql-lsp-escaped-keyword-hover");
    let source_path = temp.write(
        "escaped.ql",
        r#"
fn main() -> Int {
    let `type` = 1
    return `type`
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    let hover = hover_via_request(
        &mut service,
        uri,
        offset_to_position(&source, nth_offset(&source, "type", 2)),
    )
    .await
    .expect("escaped identifier should still have local semantic hover");
    let markup = hover_markup(hover);
    assert!(
        !markup.contains("keyword `type`"),
        "escaped identifier hover must not be keyword docs: {markup}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_self_receiver_hover_takes_precedence_over_keyword_docs() {
    let temp = TempDir::new("ql-lsp-self-hover");
    let source_path = temp.write(
        "self.ql",
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self) -> Int {
        return self.value
    }
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    let hover = hover_via_request(
        &mut service,
        uri,
        offset_to_position(&source, nth_offset(&source, "self", 2)),
    )
    .await
    .expect("self receiver should return semantic hover");
    let markup = hover_markup(hover);
    assert!(
        markup.contains("**receiver** `self`"),
        "self receiver should use semantic hover: {markup}",
    );
    assert!(
        !markup.contains("keyword `self`"),
        "semantic self hover should not be shadowed by keyword docs: {markup}",
    );
}
