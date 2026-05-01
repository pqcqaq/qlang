mod common;

use common::request::initialize_service;
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{
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

    let signature = capabilities
        .signature_help_provider
        .as_ref()
        .expect("signatureHelp provider should be declared");
    assert_eq!(
        signature.trigger_characters,
        Some(["(", ",", "<"].into_iter().map(str::to_owned).collect())
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
        capabilities.semantic_tokens_provider,
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(options))
            if options.range == Some(true)
                && options.full == Some(SemanticTokensFullOptions::Bool(true))
    ));
}
