use crate::common::request::{
    TempDir, document_highlight_via_request, goto_declaration_via_request,
    goto_definition_via_request, goto_type_definition_via_request, hover_via_request,
    inlay_hint_via_request, nth_offset, offset_to_position, references_via_request,
    semantic_tokens_full_via_request, semantic_tokens_range_via_request,
    signature_help_via_request,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::{
    assert_declaration_targets_snippet, assert_definition_targets_snippet,
    assert_document_highlight_source, assert_parameter_hint, assert_reference_targets_snippet,
    assert_reference_targets_source, assert_semantic_token, assert_type_definition_targets_snippet,
    full_source_range, hover_markup, open_real_stdlib_workspace,
};
use tower_lsp::lsp_types::{
    Range, SemanticTokenType, SemanticTokensRangeResult, SemanticTokensResult,
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
