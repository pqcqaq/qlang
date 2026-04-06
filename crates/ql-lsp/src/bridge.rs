use std::{collections::HashMap, fs};

use ql_analysis::{
    Analysis, AsyncOperatorKind, HoverInfo, LoopControlKind, PackageAnalysis, RenameError,
    SymbolKind,
};
use ql_diagnostics::{
    Diagnostic as CompilerDiagnostic, DiagnosticSeverity as CompilerSeverity, Label,
};
use ql_span::Span;
use tower_lsp::lsp_types::{
    CompletionItem as LspCompletionItem, CompletionItemKind, CompletionResponse,
    CompletionTextEdit, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity,
    GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent, MarkupKind, Position,
    PrepareRenameResponse, Range, SemanticToken, SemanticTokenType, SemanticTokens,
    SemanticTokensLegend, SemanticTokensResult, TextEdit, Url, WorkspaceEdit,
};

pub fn position_to_offset(source: &str, position: Position) -> Option<usize> {
    let line_starts = line_starts(source);
    let line_index = usize::try_from(position.line).ok()?;
    let line_start = *line_starts.get(line_index)?;
    let line_end = line_starts
        .get(line_index + 1)
        .copied()
        .unwrap_or(source.len());
    let content_end = trim_line_break(source, line_start, line_end);
    let target = usize::try_from(position.character).ok()?;

    let mut utf16_units = 0usize;
    for (offset, ch) in source[line_start..content_end].char_indices() {
        if utf16_units == target {
            return Some(line_start + offset);
        }

        utf16_units += ch.len_utf16();
        if utf16_units > target {
            return None;
        }
    }

    (utf16_units == target).then_some(content_end)
}

pub fn span_to_range(source: &str, span: Span) -> Range {
    Range::new(
        offset_to_position(source, span.start),
        offset_to_position(source, span.end),
    )
}

pub fn diagnostics_to_lsp(
    uri: &Url,
    source: &str,
    diagnostics: &[CompilerDiagnostic],
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic_to_lsp(uri, source, diagnostic))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsyncContextBridge {
    pub range: Range,
    pub operator: AsyncOperatorKind,
    pub in_async_function: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopControlContextBridge {
    pub range: Range,
    pub control: LoopControlKind,
    pub in_loop: bool,
}

pub fn hover_for_analysis(source: &str, analysis: &Analysis, position: Position) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let info = analysis.hover_at(offset)?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover_markdown(&info),
        }),
        range: Some(span_to_range(source, info.span)),
    })
}

pub fn hover_for_package_analysis(
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    position: Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    if let Some(info) = package.dependency_method_hover_at(analysis, offset) {
        let hover = HoverInfo {
            span: info.span,
            kind: info.kind,
            name: info.name,
            detail: info.detail,
            ty: None,
            definition_span: None,
        };
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: render_hover_markdown(&hover),
            }),
            range: Some(span_to_range(source, hover.span)),
        });
    }

    if let Some(info) = package.dependency_struct_field_hover_at(analysis, offset) {
        let hover = HoverInfo {
            span: info.span,
            kind: info.kind,
            name: info.name,
            detail: info.detail,
            ty: None,
            definition_span: None,
        };
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: render_hover_markdown(&hover),
            }),
            range: Some(span_to_range(source, hover.span)),
        });
    }

    if let Some(info) = package.dependency_variant_hover_at(analysis, source, offset) {
        let hover = HoverInfo {
            span: info.span,
            kind: info.kind,
            name: info.name,
            detail: info.detail,
            ty: None,
            definition_span: None,
        };
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: render_hover_markdown(&hover),
            }),
            range: Some(span_to_range(source, hover.span)),
        });
    }

    if let Some(info) = package.dependency_hover_at(analysis, offset) {
        let hover = HoverInfo {
            span: info.span,
            kind: info.kind,
            name: info.name,
            detail: info.detail,
            ty: None,
            definition_span: None,
        };
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: render_hover_markdown(&hover),
            }),
            range: Some(span_to_range(source, hover.span)),
        });
    }

    hover_for_analysis(source, analysis, position)
}

pub fn hover_for_dependency_variants(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let info = package.dependency_variant_hover_in_source_at(source, offset)?;
    let hover = HoverInfo {
        span: info.span,
        kind: info.kind,
        name: info.name,
        detail: info.detail,
        ty: None,
        definition_span: None,
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover_markdown(&hover),
        }),
        range: Some(span_to_range(source, hover.span)),
    })
}

pub fn hover_for_dependency_struct_fields(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let info = package.dependency_struct_field_hover_in_source_at(source, offset)?;
    let hover = HoverInfo {
        span: info.span,
        kind: info.kind,
        name: info.name,
        detail: info.detail,
        ty: None,
        definition_span: None,
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover_markdown(&hover),
        }),
        range: Some(span_to_range(source, hover.span)),
    })
}

pub fn hover_for_dependency_methods(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let info = package.dependency_method_hover_in_source_at(source, offset)?;
    let hover = HoverInfo {
        span: info.span,
        kind: info.kind,
        name: info.name,
        detail: info.detail,
        ty: None,
        definition_span: None,
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover_markdown(&hover),
        }),
        range: Some(span_to_range(source, hover.span)),
    })
}

pub fn hover_for_dependency_imports(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let info = package.dependency_hover_in_source_at(source, offset)?;
    let hover = HoverInfo {
        span: info.span,
        kind: info.kind,
        name: info.name,
        detail: info.detail,
        ty: None,
        definition_span: None,
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: render_hover_markdown(&hover),
        }),
        range: Some(span_to_range(source, hover.span)),
    })
}

pub fn async_context_for_analysis(
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<AsyncContextBridge> {
    analysis
        .async_context_at(position_to_offset(source, position)?)
        .map(|context| AsyncContextBridge {
            range: span_to_range(source, context.span),
            operator: context.operator,
            in_async_function: context.in_async_function,
        })
}

pub fn loop_control_context_for_analysis(
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<LoopControlContextBridge> {
    analysis
        .loop_control_context_at(position_to_offset(source, position)?)
        .map(|context| LoopControlContextBridge {
            range: span_to_range(source, context.span),
            control: context.control,
            in_loop: context.in_loop,
        })
}

pub fn definition_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = analysis.definition_at(offset)?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        span_to_range(source, target.span),
    )))
}

pub fn definition_for_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    if let Some(target) = package.dependency_method_definition_at(analysis, offset) {
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        )));
    }

    if let Some(target) = package.dependency_struct_field_definition_at(analysis, offset) {
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        )));
    }

    if let Some(target) = package.dependency_variant_definition_at(analysis, source, offset) {
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        )));
    }

    if let Some(target) = package.dependency_definition_at(analysis, offset) {
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        )));
    }

    definition_for_analysis(uri, source, analysis, position)
}

pub fn definition_for_dependency_variants(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_variant_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn definition_for_dependency_struct_fields(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_struct_field_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn definition_for_dependency_methods(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_method_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn definition_for_dependency_imports(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn references_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let references = analysis.references_at(offset)?;

    Some(
        references
            .into_iter()
            .filter(|reference| include_declaration || !reference.is_definition)
            .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span)))
            .collect(),
    )
}

pub fn references_for_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    if let Some(local_references) = package.dependency_method_references_at(analysis, offset) {
        let mut locations = Vec::new();
        if include_declaration {
            let target = package.dependency_method_definition_at(analysis, offset)?;
            let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
            let target_uri = Url::from_file_path(&target.path).ok()?;
            locations.push(Location::new(
                target_uri,
                span_to_range(&target_source, target.span),
            ));
        }

        locations.extend(
            local_references
                .into_iter()
                .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
        );
        return Some(locations);
    }

    if let Some(local_references) = package.dependency_struct_field_references_at(analysis, offset)
    {
        let mut locations = Vec::new();
        if include_declaration {
            let target = package.dependency_struct_field_definition_at(analysis, offset)?;
            let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
            let target_uri = Url::from_file_path(&target.path).ok()?;
            locations.push(Location::new(
                target_uri,
                span_to_range(&target_source, target.span),
            ));
        }

        locations.extend(
            local_references
                .into_iter()
                .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
        );
        return Some(locations);
    }

    if let Some(local_references) =
        package.dependency_variant_references_at(analysis, source, offset)
    {
        let mut locations = Vec::new();
        if include_declaration {
            let target = package.dependency_variant_definition_at(analysis, source, offset)?;
            let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
            let target_uri = Url::from_file_path(&target.path).ok()?;
            locations.push(Location::new(
                target_uri,
                span_to_range(&target_source, target.span),
            ));
        }

        locations.extend(
            local_references
                .into_iter()
                .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
        );
        return Some(locations);
    }

    if let Some(target) = package.dependency_target_at(analysis, offset) {
        let mut locations = Vec::new();
        if include_declaration {
            let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
            let target_uri = Url::from_file_path(&target.path).ok()?;
            locations.push(Location::new(
                target_uri,
                span_to_range(&target_source, target.definition_span),
            ));
        }

        let local_references = analysis.references_at(offset).unwrap_or_default();
        locations.extend(
            local_references
                .into_iter()
                .filter(|reference| include_declaration || !reference.is_definition)
                .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
        );
        return Some(locations);
    }

    references_for_analysis(uri, source, analysis, position, include_declaration)
}

pub fn references_for_dependency_imports(
    uri: &Url,
    source: &str,
    package: &PackageAnalysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let mut locations = Vec::new();
    if include_declaration {
        let target = package.dependency_definition_in_source_at(source, offset)?;
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        locations.push(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        ));
    }

    locations.extend(
        package
            .dependency_references_in_source_at(source, offset)?
            .into_iter()
            .filter(|reference| include_declaration || !reference.is_definition)
            .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
    );
    Some(locations)
}

pub fn references_for_dependency_variants(
    uri: &Url,
    source: &str,
    package: &PackageAnalysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let mut locations = Vec::new();
    if include_declaration {
        let target = package.dependency_variant_definition_in_source_at(source, offset)?;
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        locations.push(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        ));
    }

    locations.extend(
        package
            .dependency_variant_references_in_source_at(source, offset)?
            .into_iter()
            .filter(|reference| include_declaration || !reference.is_definition)
            .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
    );
    Some(locations)
}

pub fn references_for_dependency_struct_fields(
    uri: &Url,
    source: &str,
    package: &PackageAnalysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let mut locations = Vec::new();
    if include_declaration {
        let target = package.dependency_struct_field_definition_in_source_at(source, offset)?;
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        locations.push(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        ));
    }

    locations.extend(
        package
            .dependency_struct_field_references_in_source_at(source, offset)?
            .into_iter()
            .filter(|reference| include_declaration || !reference.is_definition)
            .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
    );
    Some(locations)
}

pub fn references_for_dependency_methods(
    uri: &Url,
    source: &str,
    package: &PackageAnalysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let local_references = package.dependency_method_references_in_source_at(source, offset)?;

    let mut locations = Vec::new();
    if include_declaration {
        let target = package.dependency_method_definition_in_source_at(source, offset)?;
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        locations.push(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        ));
    }

    locations.extend(
        local_references
            .into_iter()
            .map(|reference| Location::new(uri.clone(), span_to_range(source, reference.span))),
    );
    Some(locations)
}

pub fn completion_for_analysis(
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    completion_response(source, offset, analysis.completions_at(offset)?)
}

pub fn completion_for_dependency_imports(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let items = package.dependency_completions_at(source, offset)?;
    completion_response(source, offset, items)
}

pub fn completion_for_dependency_struct_fields(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let items = package.dependency_struct_field_completions_at(source, offset)?;
    completion_response(source, offset, items)
}

pub fn completion_for_dependency_variants(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let items = package.dependency_variant_completions_at(source, offset)?;
    completion_response(source, offset, items)
}

pub fn completion_for_package_analysis(
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    position: Position,
) -> Option<CompletionResponse> {
    if let Some(completion) = completion_for_dependency_imports(source, package, position) {
        return Some(completion);
    }

    if let Some(completion) = completion_for_dependency_struct_fields(source, package, position) {
        return Some(completion);
    }

    if let Some(completion) = completion_for_dependency_variants(source, package, position) {
        return Some(completion);
    }

    let offset = position_to_offset(source, position)?;
    completion_response(source, offset, analysis.completions_at(offset)?)
}

fn completion_response(
    source: &str,
    offset: usize,
    items: Vec<ql_analysis::CompletionItem>,
) -> Option<CompletionResponse> {
    let replace_span = completion_replace_span(source, offset);
    let prefix = source
        .get(replace_span.start..offset)
        .unwrap_or_default()
        .to_owned();
    let items = items
        .into_iter()
        .filter(|item| completion_matches_prefix(&item.label, &item.insert_text, &prefix))
        .map(|item| LspCompletionItem {
            label: item.label.clone(),
            kind: Some(completion_item_kind(item.kind)),
            detail: Some(item.detail),
            documentation: item
                .ty
                .map(|ty| tower_lsp::lsp_types::Documentation::String(format!("Type: `{ty}`"))),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                span_to_range(source, replace_span),
                item.insert_text,
            ))),
            ..Default::default()
        })
        .collect::<Vec<_>>();

    Some(CompletionResponse::Array(items))
}

pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::CLASS,
            SemanticTokenType::ENUM,
            SemanticTokenType::ENUM_MEMBER,
            SemanticTokenType::INTERFACE,
            SemanticTokenType::TYPE_PARAMETER,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::METHOD,
        ],
        token_modifiers: Vec::new(),
    }
}

pub fn semantic_tokens_for_analysis(source: &str, analysis: &Analysis) -> SemanticTokensResult {
    let mut data = Vec::new();
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;

    for token in analysis.semantic_tokens() {
        let start = offset_to_position(source, token.span.start);
        let delta_line = start.line - previous_line;
        let delta_start = if delta_line == 0 {
            start.character - previous_start
        } else {
            start.character
        };
        let length = semantic_token_length(source, token.span);
        let token_type = semantic_token_kind_index(token.kind);

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        previous_line = start.line;
        previous_start = start.character;
    }

    SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    })
}

pub fn prepare_rename_for_analysis(
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let offset = position_to_offset(source, position)?;
    let target = analysis.prepare_rename_at(offset)?;
    let placeholder = source.get(target.span.start..target.span.end)?.to_owned();

    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: span_to_range(source, target.span),
        placeholder,
    })
}

pub fn rename_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
    new_name: &str,
) -> Result<Option<WorkspaceEdit>, RenameError> {
    let Some(offset) = position_to_offset(source, position) else {
        return Ok(None);
    };
    let Some(rename) = analysis.rename_at(offset, new_name)? else {
        return Ok(None);
    };

    let edits = rename
        .edits
        .into_iter()
        .map(|edit| TextEdit::new(span_to_range(source, edit.span), edit.replacement))
        .collect::<Vec<_>>();
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Ok(Some(WorkspaceEdit::new(changes)))
}

fn diagnostic_to_lsp(uri: &Url, source: &str, diagnostic: &CompilerDiagnostic) -> Diagnostic {
    let primary = primary_label(diagnostic.labels.as_slice());
    let range = primary
        .map(|label| span_to_range(source, label.span))
        .unwrap_or_else(|| Range::new(Position::new(0, 0), Position::new(0, 0)));
    let mut message = diagnostic.message.clone();
    for note in &diagnostic.notes {
        message.push_str("\nnote: ");
        message.push_str(note);
    }

    Diagnostic {
        range,
        severity: Some(severity(diagnostic.severity)),
        source: Some("ql".to_owned()),
        message,
        related_information: related_information(uri, source, diagnostic.labels.as_slice()),
        ..Default::default()
    }
}

fn primary_label(labels: &[Label]) -> Option<&Label> {
    labels
        .iter()
        .find(|label| label.is_primary)
        .or_else(|| labels.first())
}

fn related_information(
    uri: &Url,
    source: &str,
    labels: &[Label],
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let infos = labels
        .iter()
        .filter(|label| !label.is_primary)
        .map(|label| DiagnosticRelatedInformation {
            location: Location::new(uri.clone(), span_to_range(source, label.span)),
            message: label
                .message
                .clone()
                .unwrap_or_else(|| "related span".to_owned()),
        })
        .collect::<Vec<_>>();

    (!infos.is_empty()).then_some(infos)
}

fn severity(severity: CompilerSeverity) -> DiagnosticSeverity {
    match severity {
        CompilerSeverity::Error => DiagnosticSeverity::ERROR,
        CompilerSeverity::Warning => DiagnosticSeverity::WARNING,
        CompilerSeverity::Note => DiagnosticSeverity::INFORMATION,
    }
}

fn render_hover_markdown(info: &HoverInfo) -> String {
    let mut text = format!(
        "**{}** `{}`\n\n```ql\n{}\n```",
        symbol_kind_name(info.kind),
        info.name,
        info.detail
    );

    if let Some(ty) = &info.ty {
        text.push_str(&format!("\n\nType: `{}`", ty));
    }

    text
}

fn symbol_kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Const => "const",
        SymbolKind::Static => "static",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Variant => "variant",
        SymbolKind::Trait => "trait",
        SymbolKind::TypeAlias => "type alias",
        SymbolKind::Field => "field",
        SymbolKind::Method => "method",
        SymbolKind::Local => "local",
        SymbolKind::Parameter => "parameter",
        SymbolKind::Generic => "generic",
        SymbolKind::SelfParameter => "receiver",
        SymbolKind::BuiltinType => "builtin type",
        SymbolKind::Import => "import",
    }
}

fn completion_replace_span(source: &str, offset: usize) -> Span {
    let mut start = offset.min(source.len());
    while start > 0 {
        let Some(ch) = source[..start].chars().next_back() else {
            break;
        };
        if !is_completion_identifier_char(ch) {
            break;
        }
        start -= ch.len_utf8();
    }

    let mut end = offset.min(source.len());
    while end < source.len() {
        let Some(ch) = source[end..].chars().next() else {
            break;
        };
        if !is_completion_identifier_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }

    Span::new(start, end)
}

fn completion_matches_prefix(label: &str, insert_text: &str, prefix: &str) -> bool {
    prefix.is_empty() || label.starts_with(prefix) || insert_text.starts_with(prefix)
}

const fn completion_item_kind(kind: SymbolKind) -> CompletionItemKind {
    match kind {
        SymbolKind::Function | SymbolKind::Method => CompletionItemKind::FUNCTION,
        SymbolKind::Const | SymbolKind::Static => CompletionItemKind::CONSTANT,
        SymbolKind::Struct => CompletionItemKind::STRUCT,
        SymbolKind::Enum => CompletionItemKind::ENUM,
        SymbolKind::Variant => CompletionItemKind::ENUM_MEMBER,
        SymbolKind::Trait => CompletionItemKind::INTERFACE,
        SymbolKind::TypeAlias | SymbolKind::BuiltinType => CompletionItemKind::CLASS,
        SymbolKind::Field => CompletionItemKind::FIELD,
        SymbolKind::Local | SymbolKind::Parameter | SymbolKind::SelfParameter => {
            CompletionItemKind::VARIABLE
        }
        SymbolKind::Generic => CompletionItemKind::TYPE_PARAMETER,
        SymbolKind::Import => CompletionItemKind::MODULE,
    }
}

const fn semantic_token_kind_index(kind: SymbolKind) -> u32 {
    match kind {
        SymbolKind::Import => 0,
        SymbolKind::BuiltinType | SymbolKind::TypeAlias => 1,
        SymbolKind::Struct => 2,
        SymbolKind::Enum => 3,
        SymbolKind::Variant => 4,
        SymbolKind::Trait => 5,
        SymbolKind::Generic => 6,
        SymbolKind::Parameter => 7,
        SymbolKind::Local | SymbolKind::SelfParameter => 8,
        SymbolKind::Field => 9,
        SymbolKind::Function | SymbolKind::Const | SymbolKind::Static => 10,
        SymbolKind::Method => 11,
    }
}

fn semantic_token_length(source: &str, span: Span) -> u32 {
    source[span.start..span.end]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum()
}

fn is_completion_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '`' || ch.is_ascii_alphanumeric()
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let line_starts = line_starts(source);
    let clamped = offset.min(source.len());
    let line = line_starts
        .partition_point(|line_start| *line_start <= clamped)
        .saturating_sub(1);
    let line_start = line_starts[line];
    let character = source[line_start..clamped]
        .chars()
        .map(|ch| ch.len_utf16())
        .sum::<usize>();

    Position::new(line as u32, character as u32)
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (offset, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(offset + ch.len_utf8());
        }
    }
    starts
}

fn trim_line_break(source: &str, start: usize, end: usize) -> usize {
    let mut content_end = end;
    if source.as_bytes().get(content_end.saturating_sub(1)) == Some(&b'\n') {
        content_end = content_end.saturating_sub(1);
    }
    if source.as_bytes().get(content_end.saturating_sub(1)) == Some(&b'\r') {
        content_end = content_end.saturating_sub(1);
    }
    content_end.max(start)
}
