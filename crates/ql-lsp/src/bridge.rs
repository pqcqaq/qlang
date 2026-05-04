use std::{
    collections::{HashMap, HashSet},
    fs,
};

use ql_analysis::{
    Analysis, AsyncOperatorKind, CallHierarchyItem as AnalysisCallHierarchyItem,
    DocumentSymbolTarget, HoverInfo, IncomingCall as AnalysisIncomingCall, LoopControlKind,
    OutgoingCall as AnalysisOutgoingCall, PackageAnalysis, RenameError, SymbolKind,
    TypeHierarchyItem as AnalysisTypeHierarchyItem,
};
use ql_diagnostics::{
    Diagnostic as CompilerDiagnostic, DiagnosticSeverity as CompilerSeverity, Label,
};
use ql_lexer::{TokenKind, lex};
use ql_span::Span;
use serde_json::json;
use tower_lsp::lsp_types::request::{
    GotoDeclarationResponse, GotoImplementationResponse, GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall,
    CompletionItem as LspCompletionItem, CompletionItemKind, CompletionItemTag, CompletionResponse,
    CompletionTextEdit, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity,
    DocumentSymbol, DocumentSymbolResponse, Documentation, GotoDefinitionResponse, Hover,
    HoverContents, Location, MarkupContent, MarkupKind, NumberOrString, Position,
    PrepareRenameResponse, Range, SemanticToken, SemanticTokenModifier, SemanticTokenType,
    SemanticTokens, SemanticTokensLegend, SemanticTokensResult, SymbolInformation, TextEdit,
    TypeHierarchyItem, Url, WorkspaceEdit,
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

    if let Some(hover) = hover_for_dependency_struct_fields(source, package, position) {
        return Some(hover);
    }

    if let Some(hover) = hover_for_dependency_values(source, package, position) {
        return Some(hover);
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

pub fn hover_for_dependency_values(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let info = package.dependency_value_hover_in_source_at(source, offset)?;
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
        range: None,
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

pub fn declaration_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_analysis(uri, source, analysis, position).map(definition_to_declaration)
}

pub fn type_definition_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = analysis.type_definition_at(offset)?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        span_to_range(source, target.span),
    )))
}

pub fn implementation_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<GotoImplementationResponse> {
    let offset = position_to_offset(source, position)?;
    let locations = if let Some(targets) = analysis.implementations_at(offset) {
        targets
            .into_iter()
            .map(|target| Location::new(uri.clone(), span_to_range(source, target.span)))
            .collect::<Vec<_>>()
    } else {
        let definition = analysis.definition_at(offset)?;
        if definition.kind != ql_analysis::SymbolKind::Method || definition.span.contains(offset) {
            return None;
        }
        vec![Location::new(
            uri.clone(),
            span_to_range(source, definition.span),
        )]
    };

    if locations.len() == 1 {
        Some(GotoImplementationResponse::Scalar(
            locations
                .into_iter()
                .next()
                .expect("single location exists"),
        ))
    } else {
        Some(GotoImplementationResponse::Array(locations))
    }
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

    if let Some(definition) = definition_for_dependency_struct_fields(source, package, position) {
        return Some(definition);
    }

    if let Some(definition) = definition_for_dependency_values(source, package, position) {
        return Some(definition);
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

pub fn declaration_for_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_package_analysis(uri, source, analysis, package, position)
        .map(definition_to_declaration)
}

pub fn type_definition_for_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    if let Some(definition) = type_definition_for_analysis(uri, source, analysis, position) {
        return Some(definition);
    }

    let offset = position_to_offset(source, position)?;
    let target = package
        .dependency_type_definition_at(analysis, offset)
        .or_else(|| package.dependency_value_type_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_variant_type_definition_at(analysis, source, offset))
        .or_else(|| package.dependency_struct_field_type_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_method_type_definition_in_source_at(source, offset))?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn type_definition_for_dependency_imports(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_type_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn type_definition_for_dependency_values(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_value_type_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn type_definition_for_dependency_variants(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_variant_type_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn type_definition_for_dependency_struct_field_types(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_struct_field_type_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn type_definition_for_dependency_method_types(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_method_type_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoTypeDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
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

pub fn declaration_for_dependency_variants(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_dependency_variants(source, package, position).map(definition_to_declaration)
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

pub fn declaration_for_dependency_struct_fields(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_dependency_struct_fields(source, package, position)
        .map(definition_to_declaration)
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

pub fn declaration_for_dependency_methods(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_dependency_methods(source, package, position).map(definition_to_declaration)
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

pub fn declaration_for_dependency_imports(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_dependency_imports(source, package, position).map(definition_to_declaration)
}

pub fn definition_for_dependency_values(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_value_definition_in_source_at(source, offset)?;
    let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
    let target_uri = Url::from_file_path(&target.path).ok()?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        target_uri,
        span_to_range(&target_source, target.span),
    )))
}

pub fn declaration_for_dependency_values(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    definition_for_dependency_values(source, package, position).map(definition_to_declaration)
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

    if let Some(references) =
        references_for_dependency_struct_fields(uri, source, package, position, include_declaration)
    {
        return Some(references);
    }

    if let Some(references) =
        references_for_dependency_values(uri, source, package, position, include_declaration)
    {
        return Some(references);
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

pub fn call_hierarchy_prepare_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<Vec<CallHierarchyItem>> {
    let offset = position_to_offset(source, position)?;
    let item = analysis.call_hierarchy_item_at(offset)?;
    Some(vec![call_hierarchy_item(uri, source, item)])
}

pub fn call_hierarchy_incoming_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<Vec<CallHierarchyIncomingCall>> {
    let offset = position_to_offset(source, position)?;
    let calls = analysis.incoming_calls_at(offset)?;
    Some(
        calls
            .into_iter()
            .map(|call| incoming_call(uri, source, call))
            .collect(),
    )
}

pub fn call_hierarchy_outgoing_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<Vec<CallHierarchyOutgoingCall>> {
    let offset = position_to_offset(source, position)?;
    let calls = analysis.outgoing_calls_at(offset)?;
    Some(
        calls
            .into_iter()
            .map(|call| outgoing_call(uri, source, call))
            .collect(),
    )
}

pub fn type_hierarchy_prepare_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<Vec<TypeHierarchyItem>> {
    let offset = position_to_offset(source, position)?;
    let item = analysis.type_hierarchy_item_at(offset)?;
    Some(vec![type_hierarchy_item(uri, source, item)])
}

pub fn type_hierarchy_supertypes_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<Vec<TypeHierarchyItem>> {
    let offset = position_to_offset(source, position)?;
    let items = analysis.supertypes_at(offset)?;
    Some(
        items
            .into_iter()
            .map(|item| type_hierarchy_item(uri, source, item))
            .collect(),
    )
}

pub fn type_hierarchy_subtypes_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<Vec<TypeHierarchyItem>> {
    let offset = position_to_offset(source, position)?;
    let items = analysis.subtypes_at(offset)?;
    Some(
        items
            .into_iter()
            .map(|item| type_hierarchy_item(uri, source, item))
            .collect(),
    )
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

pub fn references_for_dependency_values(
    uri: &Url,
    source: &str,
    package: &PackageAnalysis,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let mut locations = Vec::new();
    if include_declaration {
        let target = package.dependency_value_definition_in_source_at(source, offset)?;
        let target_source = fs::read_to_string(&target.path).ok()?.replace("\r\n", "\n");
        let target_uri = Url::from_file_path(&target.path).ok()?;
        locations.push(Location::new(
            target_uri,
            span_to_range(&target_source, target.span),
        ));
    }

    locations.extend(
        package
            .dependency_value_references_in_source_at(source, offset)?
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

pub fn completion_for_dependency_member_fields(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let items = package.dependency_member_field_completions_at(source, offset)?;
    completion_response(source, offset, items)
}

pub fn completion_for_dependency_methods(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let items = package.dependency_method_completions_at(source, offset)?;
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

    if let Some(completion) = completion_for_dependency_member_fields(source, package, position) {
        return Some(completion);
    }

    if let Some(completion) = completion_for_dependency_methods(source, package, position) {
        return Some(completion);
    }

    if let Some(completion) = completion_for_dependency_variants(source, package, position) {
        return Some(completion);
    }

    let offset = position_to_offset(source, position)?;
    completion_response(source, offset, analysis.completions_at(offset)?)
}

pub(crate) fn completion_response(
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
        .map(|item| {
            let compatibility = stdlib_compat_completion(&item);
            LspCompletionItem {
                label: item.label.clone(),
                kind: Some(completion_item_kind(item.kind)),
                detail: Some(item.detail.clone()),
                documentation: completion_documentation(&item, compatibility),
                data: completion_item_data(&item),
                sort_text: compatibility.map(|_| format!("zz_{}", item.label)),
                tags: compatibility.map(|_| vec![CompletionItemTag::DEPRECATED]),
                deprecated: compatibility.map(|_| true),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                    span_to_range(source, replace_span),
                    item.insert_text,
                ))),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return None;
    }

    Some(CompletionResponse::Array(items))
}

fn completion_documentation(
    item: &ql_analysis::CompletionItem,
    compatibility: Option<&'static str>,
) -> Option<Documentation> {
    completion_documentation_from_parts_with_note(&item.detail, item.ty.as_deref(), compatibility)
}

pub(crate) fn completion_documentation_from_parts(
    detail: &str,
    ty: Option<&str>,
) -> Option<Documentation> {
    completion_documentation_from_parts_with_note(detail, ty, None)
}

fn completion_documentation_from_parts_with_note(
    detail: &str,
    ty: Option<&str>,
    note: Option<&str>,
) -> Option<Documentation> {
    let mut sections = Vec::new();

    if !detail.trim().is_empty() {
        sections.push(format!("```ql\n{detail}\n```"));
    }

    if let Some(ty) = ty {
        sections.push(format!("Type: `{ty}`"));
    }

    if let Some(note) = note {
        sections.push(note.to_owned());
    }

    (!sections.is_empty()).then(|| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: sections.join("\n\n"),
        })
    })
}

fn stdlib_compat_completion(item: &ql_analysis::CompletionItem) -> Option<&'static str> {
    let package = item.source_package.as_deref()?;
    match package {
        "std.option" if is_std_option_compat_completion(&item.label) => Some(
            "Compatibility API. Prefer generic `Option[T]` helpers such as `some`, `none_option`, `unwrap_or`, `is_some`, `is_none`, and `or_option`.",
        ),
        "std.result" if is_std_result_compat_completion(&item.label) => Some(
            "Compatibility API. Prefer generic `Result[T, E]` helpers such as `ok`, `err`, `unwrap_result_or`, `or_result`, `error_or`, `ok_or`, `to_option`, and `error_to_option`.",
        ),
        "std.array" if is_std_array_fixed_arity_completion(&item.label) => Some(
            "Compatibility API. Prefer length-generic `std.array` helpers such as `first_array`, `last_array`, `at_array_or`, `contains_array`, `count_array`, `len_array`, `sum_int_array`, `product_int_array`, `max_int_array`, `min_int_array`, `all_bool_array`, `any_bool_array`, and `none_bool_array`.",
        ),
        _ => None,
    }
}

fn is_std_option_compat_completion(label: &str) -> bool {
    matches!(
        label,
        "IntOption"
            | "BoolOption"
            | "some_int"
            | "none_int"
            | "is_some_int"
            | "is_none_int"
            | "unwrap_or_int"
            | "or_int"
            | "or_option_int"
            | "value_or_zero_int"
            | "some_bool"
            | "none_bool"
            | "is_some_bool"
            | "is_none_bool"
            | "unwrap_or_bool"
            | "or_option_bool"
            | "value_or_false_bool"
            | "value_or_true_bool"
    )
}

fn is_std_result_compat_completion(label: &str) -> bool {
    matches!(
        label,
        "IntResult"
            | "BoolResult"
            | "ok_int"
            | "err_int"
            | "is_ok_int"
            | "is_err_int"
            | "unwrap_result_or_int"
            | "or_result_int"
            | "error_or_zero_int"
            | "error_to_option_int"
            | "ok_bool"
            | "err_bool"
            | "is_ok_bool"
            | "is_err_bool"
            | "unwrap_result_or_bool"
            | "or_result_bool"
            | "error_or_zero_bool"
            | "error_to_option_bool"
            | "ok_or_int"
            | "ok_or_bool"
            | "to_option_int"
            | "to_option_bool"
    )
}

fn is_std_array_fixed_arity_completion(label: &str) -> bool {
    let Some(stem) = label.strip_suffix("_array") else {
        return false;
    };

    ["3", "4", "5"].iter().any(|arity| {
        stem.ends_with(arity)
            || stem.ends_with(&format!("{arity}_int"))
            || stem.ends_with(&format!("{arity}_bool"))
    })
}

fn completion_item_data(item: &ql_analysis::CompletionItem) -> Option<serde_json::Value> {
    if item.detail.trim().is_empty() && item.ty.is_none() {
        return None;
    }

    Some(json!({
        "detail": item.detail,
        "ty": item.ty,
    }))
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
            SemanticTokenType::KEYWORD,
            SemanticTokenType::MODIFIER,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::STATIC,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::ASYNC,
            SemanticTokenModifier::new("unsafe"),
        ],
    }
}

pub fn semantic_tokens_for_analysis(source: &str, analysis: &Analysis) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(source, analysis.semantic_tokens(), None, true)
}

pub fn semantic_tokens_for_analysis_range(
    source: &str,
    analysis: &Analysis,
    range: Range,
) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(source, analysis.semantic_tokens(), Some(range), true)
}

pub fn semantic_tokens_for_package_analysis(
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
) -> SemanticTokensResult {
    let mut tokens = analysis.semantic_tokens();
    let dependency_import_root_tokens =
        package.dependency_import_root_semantic_tokens_in_source(source);
    let dependency_import_root_spans = dependency_import_root_tokens
        .iter()
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();
    tokens.retain(|token| {
        token.kind != SymbolKind::Import
            || !dependency_import_root_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(package.dependency_semantic_tokens_in_source(source));
    tokens.extend(dependency_import_root_tokens);
    tokens.sort_by_key(|token| {
        (
            token.span.start,
            token.span.end,
            semantic_token_kind_index(token.kind),
        )
    });
    tokens.dedup_by(|left, right| left.span == right.span && left.kind == right.kind);
    semantic_tokens_result_for_occurrences(source, tokens, None, true)
}

pub fn semantic_tokens_for_package_analysis_range(
    source: &str,
    analysis: &Analysis,
    package: &PackageAnalysis,
    range: Range,
) -> SemanticTokensResult {
    let mut tokens = analysis.semantic_tokens();
    let dependency_import_root_tokens =
        package.dependency_import_root_semantic_tokens_in_source(source);
    let dependency_import_root_spans = dependency_import_root_tokens
        .iter()
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();
    tokens.retain(|token| {
        token.kind != SymbolKind::Import
            || !dependency_import_root_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(package.dependency_semantic_tokens_in_source(source));
    tokens.extend(dependency_import_root_tokens);
    tokens.sort_by_key(|token| {
        (
            token.span.start,
            token.span.end,
            semantic_token_kind_index(token.kind),
        )
    });
    tokens.dedup_by(|left, right| left.span == right.span && left.kind == right.kind);
    semantic_tokens_result_for_occurrences(source, tokens, Some(range), true)
}

pub fn semantic_tokens_for_dependency_fallback(
    source: &str,
    package: &PackageAnalysis,
) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(
        source,
        package.dependency_fallback_semantic_tokens_in_source(source),
        None,
        true,
    )
}

pub fn semantic_tokens_for_dependency_fallback_range(
    source: &str,
    package: &PackageAnalysis,
    range: Range,
) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(
        source,
        package.dependency_fallback_semantic_tokens_in_source(source),
        Some(range),
        true,
    )
}

pub fn document_symbols_for_analysis(source: &str, analysis: &Analysis) -> DocumentSymbolResponse {
    analysis
        .document_symbols()
        .into_iter()
        .map(|symbol| document_symbol(source, symbol))
        .collect::<Vec<_>>()
        .into()
}

pub fn workspace_symbols_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    query: &str,
) -> Vec<SymbolInformation> {
    let query = query.trim().to_ascii_lowercase();
    let mut symbols = analysis
        .document_symbols()
        .into_iter()
        .filter(|symbol| {
            query.is_empty() || symbol.name.to_ascii_lowercase().contains(query.as_str())
        })
        .map(|symbol| {
            symbol_information(
                symbol.name,
                document_symbol_kind(symbol.kind),
                Location::new(uri.clone(), span_to_range(source, symbol.span)),
                None,
            )
        })
        .collect::<Vec<_>>();
    symbols.sort_by_key(|symbol| {
        (
            symbol.name.to_ascii_lowercase(),
            symbol.location.range.start.line,
            symbol.location.range.start.character,
        )
    });
    symbols
}

#[allow(deprecated)]
pub(crate) fn symbol_information(
    name: impl Into<String>,
    kind: tower_lsp::lsp_types::SymbolKind,
    location: Location,
    container_name: Option<String>,
) -> SymbolInformation {
    SymbolInformation {
        name: name.into(),
        kind,
        tags: None,
        deprecated: None,
        location,
        container_name,
    }
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

pub fn prepare_rename_for_dependency_imports(
    source: &str,
    package: &PackageAnalysis,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_prepare_rename_in_source_at(source, offset)?;
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

pub fn rename_for_dependency_imports(
    uri: &Url,
    source: &str,
    package: &PackageAnalysis,
    position: Position,
    new_name: &str,
) -> Result<Option<WorkspaceEdit>, RenameError> {
    let Some(offset) = position_to_offset(source, position) else {
        return Ok(None);
    };
    let Some(rename) = package.dependency_rename_in_source_at(source, offset, new_name)? else {
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
        code: diagnostic
            .code
            .map(|code| NumberOrString::String(code.to_owned())),
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

fn definition_to_declaration(response: GotoDefinitionResponse) -> GotoDeclarationResponse {
    match response {
        GotoDefinitionResponse::Scalar(location) => GotoDeclarationResponse::Scalar(location),
        GotoDefinitionResponse::Array(locations) => GotoDeclarationResponse::Array(locations),
        GotoDefinitionResponse::Link(links) => GotoDeclarationResponse::Link(links),
    }
}

fn call_hierarchy_item(
    uri: &Url,
    source: &str,
    item: AnalysisCallHierarchyItem,
) -> CallHierarchyItem {
    CallHierarchyItem {
        name: item.name,
        kind: document_symbol_kind(item.kind),
        tags: None,
        detail: Some(item.detail),
        uri: uri.clone(),
        range: span_to_range(source, item.span),
        selection_range: span_to_range(source, item.selection_span),
        data: None,
    }
}

fn incoming_call(uri: &Url, source: &str, call: AnalysisIncomingCall) -> CallHierarchyIncomingCall {
    CallHierarchyIncomingCall {
        from: call_hierarchy_item(uri, source, call.from),
        from_ranges: call
            .from_spans
            .into_iter()
            .map(|span| span_to_range(source, span))
            .collect(),
    }
}

fn outgoing_call(uri: &Url, source: &str, call: AnalysisOutgoingCall) -> CallHierarchyOutgoingCall {
    CallHierarchyOutgoingCall {
        to: call_hierarchy_item(uri, source, call.to),
        from_ranges: call
            .from_spans
            .into_iter()
            .map(|span| span_to_range(source, span))
            .collect(),
    }
}

fn type_hierarchy_item(
    uri: &Url,
    source: &str,
    item: AnalysisTypeHierarchyItem,
) -> TypeHierarchyItem {
    TypeHierarchyItem {
        name: item.name,
        kind: document_symbol_kind(item.kind),
        tags: None,
        detail: Some(item.detail),
        uri: uri.clone(),
        range: span_to_range(source, item.span),
        selection_range: span_to_range(source, item.selection_span),
        data: None,
    }
}

#[allow(deprecated)]
fn document_symbol(source: &str, symbol: DocumentSymbolTarget) -> DocumentSymbol {
    let range = span_to_range(source, symbol.span);
    DocumentSymbol {
        name: symbol.name,
        detail: Some(symbol.detail),
        kind: document_symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
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

pub(crate) const fn document_symbol_kind(kind: SymbolKind) -> tower_lsp::lsp_types::SymbolKind {
    match kind {
        SymbolKind::Function => tower_lsp::lsp_types::SymbolKind::FUNCTION,
        SymbolKind::Const | SymbolKind::Static => tower_lsp::lsp_types::SymbolKind::CONSTANT,
        SymbolKind::Struct => tower_lsp::lsp_types::SymbolKind::STRUCT,
        SymbolKind::Enum => tower_lsp::lsp_types::SymbolKind::ENUM,
        SymbolKind::Variant => tower_lsp::lsp_types::SymbolKind::ENUM_MEMBER,
        SymbolKind::Trait => tower_lsp::lsp_types::SymbolKind::INTERFACE,
        SymbolKind::TypeAlias | SymbolKind::BuiltinType => tower_lsp::lsp_types::SymbolKind::CLASS,
        SymbolKind::Field => tower_lsp::lsp_types::SymbolKind::FIELD,
        SymbolKind::Method => tower_lsp::lsp_types::SymbolKind::METHOD,
        SymbolKind::Local | SymbolKind::Parameter | SymbolKind::SelfParameter => {
            tower_lsp::lsp_types::SymbolKind::VARIABLE
        }
        SymbolKind::Generic => tower_lsp::lsp_types::SymbolKind::TYPE_PARAMETER,
        SymbolKind::Import => tower_lsp::lsp_types::SymbolKind::NAMESPACE,
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
        SymbolKind::Function => CompletionItemKind::FUNCTION,
        SymbolKind::Method => CompletionItemKind::METHOD,
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

#[derive(Clone, Copy)]
struct LspSemanticTokenOccurrence {
    span: Span,
    token_type: u32,
    token_modifiers_bitset: u32,
}

fn semantic_tokens_result(
    source: &str,
    tokens: Vec<LspSemanticTokenOccurrence>,
) -> SemanticTokensResult {
    let mut data = Vec::new();
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;

    for token in tokens {
        let start = offset_to_position(source, token.span.start);
        let delta_line = start.line - previous_line;
        let delta_start = if delta_line == 0 {
            start.character - previous_start
        } else {
            start.character
        };
        let length = semantic_token_length(source, token.span);

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: token.token_type,
            token_modifiers_bitset: token.token_modifiers_bitset,
        });

        previous_line = start.line;
        previous_start = start.character;
    }

    SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    })
}

pub fn semantic_tokens_result_from_occurrences(
    source: &str,
    tokens: Vec<ql_analysis::SemanticTokenOccurrence>,
) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(source, tokens, None, false)
}

pub fn semantic_tokens_result_from_occurrences_with_lexical(
    source: &str,
    tokens: Vec<ql_analysis::SemanticTokenOccurrence>,
) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(source, tokens, None, true)
}

pub fn semantic_tokens_result_from_occurrences_with_lexical_range(
    source: &str,
    tokens: Vec<ql_analysis::SemanticTokenOccurrence>,
    range: Range,
) -> SemanticTokensResult {
    semantic_tokens_result_for_occurrences(source, tokens, Some(range), true)
}

fn semantic_tokens_result_for_occurrences(
    source: &str,
    tokens: Vec<ql_analysis::SemanticTokenOccurrence>,
    range: Option<Range>,
    include_lexical_tokens: bool,
) -> SemanticTokensResult {
    let filter = range.and_then(|range| {
        Some((
            position_to_offset(source, range.start)?,
            position_to_offset(source, range.end)?,
        ))
    });
    let symbol_tokens = tokens
        .into_iter()
        .map(|token| LspSemanticTokenOccurrence {
            span: token.span,
            token_type: semantic_token_kind_index(token.kind),
            token_modifiers_bitset: semantic_token_modifier_bitset(token.kind),
        })
        .collect::<Vec<_>>();
    let symbol_spans = symbol_tokens
        .iter()
        .map(|token| token.span)
        .collect::<Vec<_>>();
    let mut entries = symbol_tokens;
    if include_lexical_tokens {
        entries.extend(lexical_semantic_tokens(source).into_iter().filter(|token| {
            !symbol_spans
                .iter()
                .any(|span| spans_overlap(*span, token.span))
        }));
    }
    if let Some((start, end)) = filter {
        entries.retain(|token| token.span.start < end && token.span.end > start);
    }
    entries.retain(|token| !token.span.is_empty());
    entries.sort_by_key(|token| (token.span.start, token.span.end, token.token_type));
    entries.dedup_by(|left, right| {
        left.span == right.span
            && left.token_type == right.token_type
            && left.token_modifiers_bitset == right.token_modifiers_bitset
    });
    semantic_tokens_result(source, entries)
}

fn lexical_semantic_tokens(source: &str) -> Vec<LspSemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    tokens
        .into_iter()
        .filter_map(|token| {
            let token_type = lexical_semantic_token_type(token.kind)?;
            Some(LspSemanticTokenOccurrence {
                span: token.span,
                token_type,
                token_modifiers_bitset: lexical_semantic_token_modifiers(token.kind),
            })
        })
        .collect()
}

fn lexical_semantic_token_type(kind: TokenKind) -> Option<u32> {
    Some(match kind {
        TokenKind::Package
        | TokenKind::Use
        | TokenKind::Const
        | TokenKind::Static
        | TokenKind::Let
        | TokenKind::Var
        | TokenKind::Fn
        | TokenKind::Await
        | TokenKind::Spawn
        | TokenKind::Defer
        | TokenKind::Return
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::Match
        | TokenKind::For
        | TokenKind::While
        | TokenKind::Loop
        | TokenKind::In
        | TokenKind::Where
        | TokenKind::Struct
        | TokenKind::Data
        | TokenKind::Enum
        | TokenKind::Trait
        | TokenKind::Impl
        | TokenKind::Extend
        | TokenKind::Type
        | TokenKind::Opaque
        | TokenKind::Extern
        | TokenKind::Is
        | TokenKind::As
        | TokenKind::Satisfies
        | TokenKind::NoneKw
        | TokenKind::TrueKw
        | TokenKind::FalseKw
        | TokenKind::MoveKw => 12,
        TokenKind::Pub | TokenKind::Async | TokenKind::Unsafe => 13,
        TokenKind::String | TokenKind::FormatString => 14,
        TokenKind::Int => 15,
        TokenKind::Arrow
        | TokenKind::FatArrow
        | TokenKind::Question
        | TokenKind::Eq
        | TokenKind::EqEq
        | TokenKind::AmpAmp
        | TokenKind::PipePipe
        | TokenKind::Bang
        | TokenKind::BangEq
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::LtEq
        | TokenKind::GtEq => 16,
        _ => return None,
    })
}

fn semantic_token_modifier_bitset(kind: SymbolKind) -> u32 {
    match kind {
        SymbolKind::Const | SymbolKind::Static => 1 << 2,
        _ => 0,
    }
}

fn lexical_semantic_token_modifiers(kind: TokenKind) -> u32 {
    match kind {
        TokenKind::Const | TokenKind::Let | TokenKind::Var | TokenKind::Fn => 1,
        TokenKind::Static => (1 << 0) | (1 << 1),
        TokenKind::Async => 1 << 3,
        TokenKind::Unsafe => 1 << 4,
        _ => 0,
    }
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
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
