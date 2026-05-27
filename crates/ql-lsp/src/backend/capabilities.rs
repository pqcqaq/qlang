use serde_json::json;
use tower_lsp::lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionProviderCapability,
    CodeLensOptions, CompletionOptions, DeclarationCapability, DocumentLinkOptions,
    DocumentOnTypeFormattingOptions, FoldingRangeProviderCapability, HoverProviderCapability,
    ImplementationProviderCapability, InitializeResult, OneOf, RenameOptions,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SignatureHelpOptions,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TypeDefinitionProviderCapability,
};

use crate::bridge::semantic_tokens_legend;

pub(super) fn initialize_result(supports_dynamic_type_hierarchy: bool) -> InitializeResult {
    InitializeResult {
        server_info: Some(ServerInfo {
            name: "qlsp".to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        capabilities: server_capabilities(supports_dynamic_type_hierarchy),
    }
}

fn server_capabilities(supports_dynamic_type_hierarchy: bool) -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        declaration_provider: Some(DeclarationCapability::Simple(true)),
        type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
        implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        document_link_provider: Some(DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: Default::default(),
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
        code_lens_provider: Some(CodeLensOptions {
            resolve_provider: Some(true),
        }),
        completion_provider: Some(completion_options()),
        signature_help_provider: Some(signature_help_options()),
        code_action_provider: Some(code_action_options()),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        document_on_type_formatting_provider: Some(on_type_formatting_options()),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: semantic_tokens_legend(),
                range: Some(true),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                ..Default::default()
            },
        )),
        inlay_hint_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        experimental: Some(json!({
            "typeHierarchyProvider": true,
            "qlspDynamicTypeHierarchyProvider": supports_dynamic_type_hierarchy,
        })),
        ..Default::default()
    }
}

pub(super) fn completion_options() -> CompletionOptions {
    CompletionOptions {
        trigger_characters: Some(
            [".", ":", "\"", "/", "@", "<"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        resolve_provider: Some(true),
        ..CompletionOptions::default()
    }
}

fn signature_help_options() -> SignatureHelpOptions {
    SignatureHelpOptions {
        trigger_characters: Some(["(", ",", "<"].into_iter().map(str::to_owned).collect()),
        retrigger_characters: Some([")"].into_iter().map(str::to_owned).collect()),
        work_done_progress_options: Default::default(),
    }
}

fn code_action_options() -> CodeActionProviderCapability {
    CodeActionProviderCapability::Options(CodeActionOptions {
        code_action_kinds: Some(vec![
            CodeActionKind::QUICKFIX,
            CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
        ]),
        resolve_provider: Some(true),
        ..CodeActionOptions::default()
    })
}

fn on_type_formatting_options() -> DocumentOnTypeFormattingOptions {
    DocumentOnTypeFormattingOptions {
        first_trigger_character: "\n".to_owned(),
        more_trigger_character: Some(["}", ";", ","].into_iter().map(str::to_owned).collect()),
    }
}
