use crate::common::request::{
    TempDir, did_open_via_request, document_highlight_via_request, folding_range_via_request,
    formatting_via_request, goto_declaration_via_request, goto_definition_via_request,
    goto_type_definition_via_request, hover_via_request, inlay_hint_via_request, nth_offset,
    offset_to_position, on_type_formatting_via_request, range_formatting_via_request,
    references_via_request, selection_range_via_request, semantic_tokens_full_via_request,
    semantic_tokens_range_via_request, signature_help_via_request,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::{
    assert_declaration_targets_snippet, assert_definition_targets_snippet,
    assert_document_highlight_source, assert_folding_range_starts_at_source_line,
    assert_parameter_hint, assert_reference_targets_snippet, assert_reference_targets_source,
    assert_selection_range_source, assert_semantic_token, assert_type_definition_targets_snippet,
    full_source_range, hover_markup, open_real_stdlib_workspace, range_for,
};
use tower_lsp::lsp_types::{
    Position, Range, SemanticTokenType, SemanticTokensRangeResult, SemanticTokensResult, TextEdit,
    Url,
};

#[tokio::test(flavor = "current_thread")]
async fn rich_requests_use_current_real_stdlib_interfaces() {
    let temp = TempDir::new("ql-lsp-real-stdlib-rich-requests");
    let app_source = r#"
package demo.app

use std.option.Option as Option
use std.option.some as option_some
use std.result.Result as Result
use std.result.ok_or as result_ok_or
use std.array.repeat_array as repeat_array
use std.core.clamp_int as clamp_int
use std.test.expect_eq as expect_eq

pub fn main() -> Int {
    let option_value: Option[Int] = option_some(42)
    let result_value: Result[Int, Int] = result_ok_or(option_value, 5)
    let values: [Int; 3] = repeat_array(2)
    let check = expect_eq(clamp_int(42, 0, 100), 42)
    return check
}
"#;
    let (mut service, app_uri, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;

    let option_hover = hover_markup(
        hover_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(
                app_source,
                nth_offset(app_source, ": Option", 1) + ": ".len(),
            ),
        )
        .await
        .expect("real std.option hover request should return markup"),
    );
    assert!(option_hover.contains("**enum** `Option`"));
    assert!(option_hover.contains("enum Option[T]"));
    assert!(!option_hover.contains("Compatibility API"));

    assert_definition_targets_snippet(
        goto_definition_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
        )
        .await
        .expect("real std.option function definition should exist"),
        &real_stdlib_source_path(&stdlib_root, "option"),
        "some",
    );
    assert_declaration_targets_snippet(
        goto_declaration_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
        )
        .await
        .expect("real std.option function declaration should exist"),
        &real_stdlib_source_path(&stdlib_root, "option"),
        "some",
    );
    let option_references = references_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
        true,
    )
    .await
    .expect("real std.option references should exist");
    assert_reference_targets_snippet(
        &option_references,
        &real_stdlib_source_path(&stdlib_root, "option"),
        "some",
    );
    assert_reference_targets_source(
        &option_references,
        &app_uri,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 1),
    );
    assert_reference_targets_source(
        &option_references,
        &app_uri,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 2),
    );

    let option_highlights = document_highlight_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
    )
    .await
    .expect("real stdlib documentHighlight should return current-file highlights");
    assert_document_highlight_source(
        &option_highlights,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 1),
    );
    assert_document_highlight_source(
        &option_highlights,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 2),
    );

    assert_type_definition_targets_snippet(
        goto_type_definition_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(
                app_source,
                nth_offset(app_source, ": Option", 1) + ": ".len(),
            ),
        )
        .await
        .expect("real std.option type definition should exist"),
        &real_stdlib_source_path(&stdlib_root, "option"),
        "Option",
    );

    let signature = signature_help_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            app_source,
            nth_offset(app_source, "result_ok_or(option_value, ", 1)
                + "result_ok_or(option_value, ".len(),
        ),
    )
    .await
    .expect("real std.result signatureHelp should return a signature");
    assert_eq!(signature.active_parameter, Some(1));
    assert_eq!(
        signature.signatures[0].label,
        "fn ok_or[T, E](value: Option[T], error: E) -> Result[T, E]"
    );

    let hints =
        inlay_hint_via_request(&mut service, app_uri.clone(), full_source_range(app_source))
            .await
            .expect("real stdlib inlayHint should return parameter hints");
    assert_parameter_hint(&hints, "value:");
    assert_parameter_hint(&hints, "error:");
    assert_parameter_hint(&hints, "low:");
    assert_parameter_hint(&hints, "high:");
    assert_parameter_hint(&hints, "actual:");
    assert_parameter_hint(&hints, "expected:");

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_full_via_request(&mut service, app_uri.clone())
            .await
            .expect("real stdlib semantic tokens request should return tokens")
    else {
        panic!("semantic tokens should use full token payload")
    };
    assert_semantic_token(
        app_source,
        &tokens.data,
        nth_offset(app_source, ": Option", 1) + ": ".len(),
        "Option".len(),
        SemanticTokenType::ENUM,
    );
    assert_semantic_token(
        app_source,
        &tokens.data,
        nth_offset(app_source, "result_ok_or", 2),
        "result_ok_or".len(),
        SemanticTokenType::FUNCTION,
    );
    assert_semantic_token(
        app_source,
        &tokens.data,
        nth_offset(app_source, "clamp_int", 2),
        "clamp_int".len(),
        SemanticTokenType::FUNCTION,
    );

    let SemanticTokensRangeResult::Tokens(range_tokens) = semantic_tokens_range_via_request(
        &mut service,
        app_uri,
        Range::new(
            offset_to_position(app_source, nth_offset(app_source, "let option_value", 1)),
            offset_to_position(app_source, nth_offset(app_source, "return check", 1)),
        ),
    )
    .await
    .expect("real stdlib semantic tokens range request should return tokens") else {
        panic!("semantic tokens range should use full token payload")
    };
    assert_semantic_token(
        app_source,
        &range_tokens.data,
        nth_offset(app_source, "repeat_array", 3),
        "repeat_array".len(),
        SemanticTokenType::FUNCTION,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn rich_requests_cover_formatting_folding_and_selection_in_real_stdlib_workspace() {
    let temp = TempDir::new("ql-lsp-real-stdlib-rich-editor-requests");
    let app_source = r#"package demo.app

use std.core.clamp_int as clamp_int

/* module fold
   stays foldable
*/
fn add(left: Int, right: Int)->Int{
return left + right
}

// group fold alpha
// group fold beta
pub fn main() -> Int {
let total= add(1, 2)
let next = clamp_int(total,0,100)
let marker: String = "not a comment
// not a line comment
/* not a block comment */
// still not a line comment"
if total > 0 {
return next
}
return 0
}
"#;
    let formatted_app_source = r#"package demo.app

use std.core.clamp_int as clamp_int

fn add(left: Int, right: Int) -> Int {
    return left + right
}

pub fn main() -> Int {
    let total = add(1, 2)
    let next = clamp_int(total, 0, 100)
    let marker: String = "not a comment
// not a line comment
/* not a block comment */
// still not a line comment"
    if total > 0 {
        return next
    }
    return 0
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;

    let formatting_edits = formatting_via_request(&mut service, app_uri.clone())
        .await
        .expect("real stdlib app formatting should return edits");
    assert_eq!(
        formatting_edits,
        vec![TextEdit::new(
            full_source_range(app_source),
            formatted_app_source.to_owned(),
        )],
        "real stdlib app formatting should normalize the whole file",
    );

    let formatting_source = r#"package demo.app

fn main()->Int{
return 1
}
"#;
    let formatting_path = temp.write("workspace/app/src/formatting.ql", formatting_source);
    let formatting_uri =
        Url::from_file_path(&formatting_path).expect("formatting path should convert to URI");
    did_open_via_request(
        &mut service,
        formatting_uri.clone(),
        formatting_source.to_owned(),
    )
    .await;

    let add_return_range = range_for(formatting_source, "return 1", 1);
    let add_return_line = add_return_range.start.line;
    let range_edits =
        range_formatting_via_request(&mut service, formatting_uri.clone(), add_return_range)
            .await
            .expect("real stdlib app rangeFormatting should return edits");
    assert_eq!(
        range_edits,
        vec![TextEdit::new(
            Range::new(
                Position::new(add_return_line, 0),
                Position::new(add_return_line, 0),
            ),
            "    ".to_owned(),
        )],
        "real stdlib app rangeFormatting should indent the body line",
    );

    let on_type_edits = on_type_formatting_via_request(
        &mut service,
        formatting_uri,
        Position::new(add_return_line, 0),
        "\n",
    )
    .await
    .expect("real stdlib app onTypeFormatting should return edits");
    assert_eq!(
        on_type_edits,
        vec![TextEdit::new(
            Range::new(
                Position::new(add_return_line, 0),
                Position::new(add_return_line, 0),
            ),
            "    ".to_owned(),
        )],
        "real stdlib app onTypeFormatting should indent the body line",
    );

    let folds = folding_range_via_request(&mut service, app_uri.clone())
        .await
        .expect("real stdlib app foldingRange should return source folds");
    assert!(
        folds.iter().any(|range| range.start_line < range.end_line),
        "foldingRange should include multiline folds: {folds:#?}",
    );
    let comment_folds = folds
        .iter()
        .filter(|range| {
            range.kind.as_ref() == Some(&tower_lsp::lsp_types::FoldingRangeKind::Comment)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        comment_folds.len(),
        2,
        "foldingRange should include only real comment folds, not string markers: {folds:#?}",
    );
    assert_folding_range_starts_at_source_line(&folds, app_source, "/* module fold", 1);
    assert_folding_range_starts_at_source_line(&folds, app_source, "// group fold alpha", 1);

    let selections = selection_range_via_request(
        &mut service,
        app_uri,
        vec![offset_to_position(
            app_source,
            nth_offset(app_source, "next", 1),
        )],
    )
    .await
    .expect("real stdlib app selectionRange should return token selection");
    assert_selection_range_source(
        &selections,
        app_source,
        "next",
        nth_offset(app_source, "next", 1),
    );
}
