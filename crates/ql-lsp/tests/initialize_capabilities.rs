mod common;

use common::request::initialize_service;
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionProviderCapability,
    FoldingRangeProviderCapability, OneOf, SelectionRangeProviderCapability,
    SemanticTokensFullOptions, SemanticTokensServerCapabilities,
};

#[tokio::test(flavor = "current_thread")]
async fn initialize_declares_rich_editor_capabilities() {
    let (mut service, _) = LspService::new(Backend::new);
    let result = initialize_service(&mut service).await;
    let capabilities = result.capabilities;

    let completion = capabilities
        .completion_provider
        .as_ref()
        .expect("completion provider should be declared");
    assert_eq!(
        completion.trigger_characters,
        Some(
            [".", ":", "\"", "/", "@", "<"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        )
    );
    assert_eq!(completion.resolve_provider, Some(true));
    let code_lens = capabilities
        .code_lens_provider
        .as_ref()
        .expect("codeLens provider should be declared");
    assert_eq!(code_lens.resolve_provider, Some(true));

    let signature = capabilities
        .signature_help_provider
        .as_ref()
        .expect("signatureHelp provider should be declared");
    assert_eq!(
        signature.trigger_characters,
        Some(["(", ",", "<"].into_iter().map(str::to_owned).collect())
    );
    assert!(matches!(
        capabilities.document_formatting_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.document_range_formatting_provider,
        Some(OneOf::Left(true))
    ));
    let Some(CodeActionProviderCapability::Options(code_action)) =
        capabilities.code_action_provider.as_ref()
    else {
        panic!(
            "codeAction provider should declare option capabilities, got {:?}",
            capabilities.code_action_provider
        )
    };
    assert_eq!(code_action.resolve_provider, Some(true));
    assert_eq!(
        code_action.code_action_kinds,
        Some(vec![
            CodeActionKind::QUICKFIX,
            CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
        ])
    );
    let on_type = capabilities
        .document_on_type_formatting_provider
        .as_ref()
        .expect("onTypeFormatting provider should be declared");
    assert_eq!(on_type.first_trigger_character, "\n");
    assert_eq!(
        on_type.more_trigger_character,
        Some(["}", ";", ","].into_iter().map(str::to_owned).collect())
    );
    assert!(matches!(
        capabilities.folding_range_provider,
        Some(FoldingRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.selection_range_provider,
        Some(SelectionRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.inlay_hint_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.call_hierarchy_provider,
        Some(CallHierarchyServerCapability::Simple(true))
    ));
    assert_eq!(
        capabilities
            .experimental
            .as_ref()
            .and_then(|value| value.get("typeHierarchyProvider"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        capabilities
            .experimental
            .as_ref()
            .and_then(|value| value.get("qlspDynamicTypeHierarchyProvider"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(matches!(
        capabilities.semantic_tokens_provider,
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(options))
            if options.range == Some(true)
                && options.full == Some(SemanticTokensFullOptions::Bool(true))
    ));
}
