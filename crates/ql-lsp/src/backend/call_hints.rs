use ql_analysis::Analysis;
use ql_span::Span;
use tower_lsp::lsp_types::{InlayHint, Position, Range, SignatureHelp, Url};

use crate::bridge::span_to_range;
use crate::editor_features::{
    inlay_hints_for_analysis, parameter_name_inlay_hints_for_callable_detail,
    signature_help_for_callable_detail,
};

use super::{
    WorkspaceRequestContext, dependency_definition_target_with_open_docs_at,
    workspace_callable_detail_for_dependency_target_with_open_docs,
};

pub(super) fn signature_help_for_workspace_context(
    uri: &Url,
    source: &str,
    context: &WorkspaceRequestContext,
    position: Position,
) -> Option<SignatureHelp> {
    signature_help_for_callable_detail(source, position, |offset| {
        workspace_callable_detail_at(uri, source, context, offset)
    })
}

pub(super) fn inlay_hints_for_workspace_context(
    uri: &Url,
    source: &str,
    context: &WorkspaceRequestContext,
    range: Range,
) -> Option<Vec<InlayHint>> {
    let mut hints = context
        .analysis
        .as_ref()
        .and_then(|analysis| inlay_hints_for_analysis(source, analysis, range))
        .unwrap_or_default();
    hints.extend(dependency_parameter_name_inlay_hints_for_workspace_context(
        uri, source, context, range,
    ));
    hints.sort_by_key(|hint| (hint.position.line, hint.position.character));
    (!hints.is_empty()).then_some(hints)
}

fn dependency_parameter_name_inlay_hints_for_workspace_context(
    uri: &Url,
    source: &str,
    context: &WorkspaceRequestContext,
    range: Range,
) -> Vec<InlayHint> {
    parameter_name_inlay_hints_for_callable_detail(source, range, |offset| {
        workspace_callable_detail_at(uri, source, context, offset)
    })
}

fn workspace_callable_detail_at(
    uri: &Url,
    source: &str,
    context: &WorkspaceRequestContext,
    offset: usize,
) -> Option<String> {
    dependency_definition_target_with_open_docs_at(
        source,
        context.analysis.as_ref(),
        &context.package,
        &context.open_docs,
        span_to_range(source, Span::new(offset, offset)).start,
    )
    .and_then(|target| {
        workspace_callable_detail_for_dependency_target_with_open_docs(
            uri,
            source,
            context.analysis.as_ref(),
            &context.package,
            &context.open_docs,
            &target,
        )
    })
    .or_else(|| {
        dependency_callable_detail_at(source, context.analysis.as_ref(), &context.package, offset)
    })
    .or_else(|| {
        context
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.hover_at(offset).map(|info| info.detail))
    })
}

fn dependency_callable_detail_at(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    offset: usize,
) -> Option<String> {
    analysis
        .and_then(|analysis| {
            package
                .dependency_method_hover_at(analysis, offset)
                .or_else(|| package.dependency_variant_hover_at(analysis, source, offset))
                .or_else(|| package.dependency_hover_at(analysis, offset))
        })
        .or_else(|| {
            package
                .dependency_method_hover_in_source_at(source, offset)
                .or_else(|| package.dependency_variant_hover_in_source_at(source, offset))
                .or_else(|| package.dependency_hover_in_source_at(source, offset))
        })
        .map(|info| info.detail)
}
