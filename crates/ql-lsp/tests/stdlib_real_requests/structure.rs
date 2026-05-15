use std::fs;

use crate::common::request::{
    TempDir, did_open_via_request, document_symbol_via_request, folding_range_via_request,
    formatting_via_request, nth_offset, offset_to_position, on_type_formatting_via_request,
    range_formatting_via_request, selection_range_via_request,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::{
    assert_document_symbol, assert_folding_range_starts_at_source_line,
    assert_selection_range_source, open_real_stdlib_workspace, range_for,
};
use tower_lsp::lsp_types::{SymbolKind, Url};

#[tokio::test(flavor = "current_thread")]
async fn structure_requests_use_current_real_stdlib_sources() {
    let temp = TempDir::new("ql-lsp-real-stdlib-structure-requests");
    let app_source = r#"
package demo.app

use std.option.Option as Option

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, _, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;
    let option_source_path = real_stdlib_source_path(&stdlib_root, "option");
    let option_source = fs::read_to_string(&option_source_path)
        .expect("temp std.option source should exist")
        .replace("\r\n", "\n");
    let option_uri = Url::from_file_path(&option_source_path)
        .expect("temp std.option source path should convert to URI");
    did_open_via_request(&mut service, option_uri.clone(), option_source.clone()).await;

    let folds = folding_range_via_request(&mut service, option_uri.clone())
        .await
        .expect("real stdlib foldingRange should return source folds");
    assert_folding_range_starts_at_source_line(&folds, &option_source, "pub enum Option", 1);
    assert_folding_range_starts_at_source_line(&folds, &option_source, "pub fn unwrap_or", 1);
    assert_folding_range_starts_at_source_line(&folds, &option_source, "return match value", 1);

    let inner_offset =
        nth_offset(&option_source, "Option.Some(inner) => inner", 1) + "Option.Some(".len();
    let selections = selection_range_via_request(
        &mut service,
        option_uri.clone(),
        vec![offset_to_position(&option_source, inner_offset + 1)],
    )
    .await
    .expect("real stdlib selectionRange should return token selection");
    assert_selection_range_source(&selections, &option_source, "inner", inner_offset);

    let edits = formatting_via_request(&mut service, option_uri.clone())
        .await
        .expect("real stdlib formatting should return an edit list for parseable source");
    assert!(
        edits.is_empty(),
        "real stdlib source should already be qfmt-stable: {edits:#?}",
    );

    let range_edits = range_formatting_via_request(
        &mut service,
        option_uri.clone(),
        range_for(&option_source, "        Option.Some(inner) => inner", 1),
    )
    .await
    .expect("real stdlib rangeFormatting should return source-local edits");
    assert!(
        range_edits.is_empty(),
        "real stdlib selected range should already be qfmt-stable: {range_edits:#?}",
    );

    let on_type_edits = on_type_formatting_via_request(
        &mut service,
        option_uri,
        offset_to_position(
            &option_source,
            nth_offset(&option_source, "        Option.None", 1),
        ),
        "\n",
    )
    .await
    .expect("real stdlib onTypeFormatting should return source-local edits");
    assert!(
        on_type_edits.is_empty(),
        "real stdlib trigger line should already be qfmt-stable: {on_type_edits:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn document_symbol_request_uses_current_real_stdlib_sources() {
    let temp = TempDir::new("ql-lsp-real-stdlib-document-symbol-request");
    let app_source = r#"
package demo.app

use std.option.Option as Option

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, _, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;
    let option_source_path = real_stdlib_source_path(&stdlib_root, "option");
    let option_source = fs::read_to_string(&option_source_path)
        .expect("temp std.option source should exist")
        .replace("\r\n", "\n");
    let option_uri = Url::from_file_path(&option_source_path)
        .expect("temp std.option source path should convert to URI");
    did_open_via_request(&mut service, option_uri.clone(), option_source.clone()).await;

    let symbols = document_symbol_via_request(&mut service, option_uri)
        .await
        .expect("real stdlib documentSymbol should return source symbols");
    assert_document_symbol(&symbols, "Option", SymbolKind::ENUM);
    assert_document_symbol(&symbols, "some", SymbolKind::FUNCTION);
    assert_document_symbol(&symbols, "unwrap_or", SymbolKind::FUNCTION);
    assert_document_symbol(&symbols, "or_option", SymbolKind::FUNCTION);
}
