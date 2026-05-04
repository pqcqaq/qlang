mod common;

use common::request::{
    TempDir, did_open_via_request, initialize_service_with_workspace_roots,
    initialized_service_with_open_documents, nth_offset, offset_to_position,
    semantic_tokens_full_via_request, semantic_tokens_range_via_request,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{
    Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensRangeResult,
    SemanticTokensResult, Url,
};

fn decode(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32)> {
    let mut line = 0u32;
    let mut start = 0u32;
    let mut decoded = Vec::new();
    for token in tokens {
        line += token.delta_line;
        if token.delta_line == 0 {
            start += token.delta_start;
        } else {
            start = token.delta_start;
        }
        decoded.push((line, start, token.length, token.token_type));
    }
    decoded
}

fn decode_with_modifiers(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
    let mut line = 0u32;
    let mut start = 0u32;
    let mut decoded = Vec::new();
    for token in tokens {
        line += token.delta_line;
        if token.delta_line == 0 {
            start += token.delta_start;
        } else {
            start = token.delta_start;
        }
        decoded.push((
            line,
            start,
            token.length,
            token.token_type,
            token.token_modifiers_bitset,
        ));
    }
    decoded
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_include_lexical_keyword_literal_and_operator_tokens() {
    let temp = TempDir::new("ql-lsp-lexical-semantic-tokens");
    let source_path = temp.write(
        "tokens.ql",
        r#"
pub fn main() -> Int {
    let value = 1 + 2
    let text = "ok"
    return value
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_full_via_request(&mut service, uri.clone())
            .await
            .expect("semanticTokens/full should return tokens")
    else {
        panic!("semanticTokens/full should return full tokens")
    };
    let decoded = decode(&tokens.data);
    let legend = ql_lsp::bridge::semantic_tokens_legend();
    let keyword_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::KEYWORD)
        .expect("keyword token type should exist") as u32;
    let modifier_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::MODIFIER)
        .expect("modifier token type should exist") as u32;
    let number_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::NUMBER)
        .expect("number token type should exist") as u32;
    let string_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::STRING)
        .expect("string token type should exist") as u32;
    let operator_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::OPERATOR)
        .expect("operator token type should exist") as u32;

    for (needle, token_type) in [
        ("pub", modifier_type),
        ("fn", keyword_type),
        ("let", keyword_type),
        ("1", number_type),
        ("+", operator_type),
        ("\"ok\"", string_type),
        ("return", keyword_type),
    ] {
        let pos = offset_to_position(&source, nth_offset(&source, needle, 1));
        assert!(
            decoded.contains(&(pos.line, pos.character, needle.len() as u32, token_type)),
            "{needle} should have token type {token_type}; decoded={decoded:#?}",
        );
    }

    let range = Range::new(
        offset_to_position(&source, nth_offset(&source, "let text", 1)),
        offset_to_position(&source, nth_offset(&source, "return", 1)),
    );
    let SemanticTokensRangeResult::Tokens(range_tokens) =
        semantic_tokens_range_via_request(&mut service, uri, range)
            .await
            .expect("semanticTokens/range should return tokens")
    else {
        panic!("semanticTokens/range should return full token data")
    };
    let range_decoded = decode(&range_tokens.data);
    let string_pos = offset_to_position(&source, nth_offset(&source, "\"ok\"", 1));
    assert!(
        range_decoded.contains(&(
            string_pos.line,
            string_pos.character,
            "\"ok\"".len() as u32,
            string_type
        )),
        "range tokens should include string token inside requested range: {range_decoded:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_range_uses_workspace_open_docs_like_full() {
    let temp = TempDir::new("ql-lsp-semantic-token-range-open-docs");
    let app_source = r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    return built.value + config.value
}
"#;
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core
"#,
    );
    let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}
"#;

    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, core_uri, open_core_source.to_owned()).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let legend = ql_lsp::bridge::semantic_tokens_legend();
    let class_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::CLASS)
        .expect("class token type should exist") as u32;
    let cfg_position = offset_to_position(app_source, nth_offset(app_source, "Cfg", 1));
    let cfg_entry = (
        cfg_position.line,
        cfg_position.character,
        "Cfg".len() as u32,
        class_type,
    );

    let SemanticTokensResult::Tokens(full_tokens) =
        semantic_tokens_full_via_request(&mut service, app_uri.clone())
            .await
            .expect("semanticTokens/full should return tokens")
    else {
        panic!("semanticTokens/full should return full tokens")
    };
    assert!(
        decode(&full_tokens.data).contains(&cfg_entry),
        "full tokens should classify Cfg from the open workspace source"
    );

    let range = Range::new(
        offset_to_position(app_source, nth_offset(app_source, "use demo.core", 1)),
        offset_to_position(app_source, nth_offset(app_source, "return", 1)),
    );
    let SemanticTokensRangeResult::Tokens(range_tokens) =
        semantic_tokens_range_via_request(&mut service, app_uri, range)
            .await
            .expect("semanticTokens/range should return tokens")
    else {
        panic!("semanticTokens/range should return token data")
    };
    assert!(
        decode(&range_tokens.data).contains(&cfg_entry),
        "range tokens should use the same open-doc workspace classification as full tokens"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_request_marks_stdlib_compat_imports_deprecated() {
    let temp = TempDir::new("ql-lsp-stdlib-compat-semantic-token-request");
    let app_source = r#"
package demo.app

use std.option.IntOption as MaybeInt
use std.option.Option as GenericOption
use std.array.sum3_int_array as sum_three
use std.array.sum_int_array as sum_any

pub fn main() -> Int {
    return 0
}
"#;
    let app_path = temp.write("workspace/app/src/main.ql", app_source);
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../option", "../array"]
"#,
    );
    temp.write(
        "workspace/option/qlang.toml",
        r#"
[package]
name = "std.option"
"#,
    );
    temp.write(
        "workspace/option/std.option.qi",
        r#"
// qlang interface v1
// package: std.option

// source: src/lib.ql
package std.option

pub enum Option[T] {
    Some(T),
    None,
}
pub enum IntOption {
    Some(Int),
    None,
}
"#,
    );
    temp.write(
        "workspace/array/qlang.toml",
        r#"
[package]
name = "std.array"
"#,
    );
    temp.write(
        "workspace/array/std.array.qi",
        r#"
// qlang interface v1
// package: std.array

// source: src/lib.ql
package std.array

pub fn sum_int_array[N](values: [Int; N]) -> Int
pub fn sum3_int_array(values: [Int; 3]) -> Int
"#,
    );

    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_full_via_request(&mut service, app_uri)
            .await
            .expect("semanticTokens/full should return tokens")
    else {
        panic!("semanticTokens/full should return full token data")
    };
    let decoded = decode_with_modifiers(&tokens.data);
    let legend = ql_lsp::bridge::semantic_tokens_legend();
    let deprecated_bit = 1u32
        << legend
            .token_modifiers
            .iter()
            .position(|modifier| *modifier == SemanticTokenModifier::DEPRECATED)
            .expect("deprecated token modifier should exist");

    for (needle, expected_deprecated) in [
        ("MaybeInt", true),
        ("GenericOption", false),
        ("sum_three", true),
        ("sum_any", false),
    ] {
        let position = offset_to_position(app_source, nth_offset(app_source, needle, 1));
        let entry = decoded
            .iter()
            .find(|(line, start, length, _, _)| {
                *line == position.line
                    && *start == position.character
                    && *length == needle.len() as u32
            })
            .unwrap_or_else(|| panic!("semantic token for `{needle}` should exist"));
        assert_eq!(
            entry.4 & deprecated_bit != 0,
            expected_deprecated,
            "`{needle}` deprecated modifier mismatch; decoded={decoded:#?}",
        );
    }
}
