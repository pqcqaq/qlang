use std::collections::{HashMap, HashSet};

use ql_analysis::{Analysis, SemanticTokenOccurrence};
use ql_lexer::{TokenKind, lex};
use ql_span::Span;
use tower_lsp::lsp_types::{Range, SemanticTokensRangeResult, SemanticTokensResult, Url};

use crate::bridge::{
    semantic_tokens_result_from_package_occurrences_with_lexical,
    semantic_tokens_result_from_package_occurrences_with_lexical_range, span_to_range,
};

use super::{
    OpenDocuments, WorkspaceRequestContext, broken_source_import_bindings_in_tokens,
    broken_source_import_token_matches_reference_context, dependency_definition_target_at,
    dependency_definition_target_with_open_docs_at, dependency_occurrence_span_at,
    dependency_occurrence_span_with_open_docs_at, same_dependency_definition_target,
    supports_workspace_import_definition, workspace_source_kind_for_import_binding_with_open_docs,
};

pub(super) fn semantic_tokens_range_result(
    result: SemanticTokensResult,
) -> SemanticTokensRangeResult {
    match result {
        SemanticTokensResult::Tokens(tokens) => SemanticTokensRangeResult::Tokens(tokens),
        SemanticTokensResult::Partial(partial) => SemanticTokensRangeResult::Partial(partial),
    }
}

pub(super) fn semantic_tokens_for_workspace_context(
    uri: &Url,
    source: &str,
    context: &WorkspaceRequestContext,
) -> SemanticTokensResult {
    match context.analysis.as_ref() {
        Some(analysis) => semantic_tokens_for_workspace_package_analysis_with_open_docs(
            uri,
            source,
            analysis,
            &context.package,
            &context.open_docs,
        ),
        None => semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
            uri,
            source,
            &context.package,
            &context.open_docs,
        ),
    }
}

pub(super) fn semantic_tokens_for_workspace_context_range(
    uri: &Url,
    source: &str,
    context: &WorkspaceRequestContext,
    range: Range,
) -> SemanticTokensResult {
    match context.analysis.as_ref() {
        Some(analysis) => semantic_tokens_for_workspace_package_analysis_range_with_open_docs(
            uri,
            source,
            analysis,
            &context.package,
            &context.open_docs,
            range,
        ),
        None => semantic_tokens_for_workspace_dependency_fallback_range_with_open_docs(
            uri,
            source,
            &context.package,
            &context.open_docs,
            range,
        ),
    }
}

#[cfg(test)]
pub(super) fn semantic_tokens_for_workspace_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
) -> SemanticTokensResult {
    let open_docs = OpenDocuments::new();
    semantic_tokens_for_workspace_package_analysis_with_open_docs(
        uri, source, analysis, package, &open_docs,
    )
}

#[cfg(test)]
pub(super) fn semantic_tokens_for_workspace_dependency_fallback(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
) -> SemanticTokensResult {
    let open_docs = OpenDocuments::new();
    semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
        uri, source, package, &open_docs,
    )
}

fn workspace_import_semantic_tokens_in_analysis_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let bindings = broken_source_import_bindings_in_tokens(&tokens);
    let mut local_name_counts = HashMap::<String, usize>::new();
    let mut local_name_kinds = HashMap::<String, ql_analysis::SymbolKind>::new();

    for binding in bindings {
        let Some(kind) = workspace_source_kind_for_import_binding_with_open_docs(
            uri,
            source,
            Some(analysis),
            package,
            open_docs,
            &binding.import_prefix,
            binding.imported_name.as_str(),
            supports_workspace_import_definition,
        ) else {
            continue;
        };
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
        local_name_kinds.insert(binding.local_name, kind);
    }

    let mut occurrences = Vec::new();
    for (local_name, kind) in local_name_kinds {
        if local_name_counts.get(local_name.as_str()) != Some(&1usize) {
            continue;
        }
        for (start, _) in source.match_indices(local_name.as_str()) {
            let Some((binding, span)) = analysis.import_binding_at(start) else {
                continue;
            };
            if binding.local_name == local_name && span.start == start {
                occurrences.push(SemanticTokenOccurrence { span, kind });
            }
        }
    }
    occurrences
}

fn workspace_import_semantic_tokens_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let mut unique_bindings = HashMap::<String, (Span, ql_analysis::SymbolKind)>::new();
    let mut local_name_counts = HashMap::<String, usize>::new();

    for binding in broken_source_import_bindings_in_tokens(&tokens) {
        let Some(kind) = workspace_source_kind_for_import_binding_with_open_docs(
            uri,
            source,
            None,
            package,
            open_docs,
            &binding.import_prefix,
            binding.imported_name.as_str(),
            supports_workspace_import_definition,
        ) else {
            continue;
        };
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
        unique_bindings.insert(binding.local_name, (binding.definition_span, kind));
    }

    tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.kind == TokenKind::Ident)
        .filter_map(|(index, token)| {
            let (definition_span, kind) = unique_bindings.get(token.text.as_str())?;
            (local_name_counts.get(token.text.as_str()) == Some(&1usize)
                && (token.span == *definition_span
                    || broken_source_import_token_matches_reference_context(&tokens, index)))
            .then_some(SemanticTokenOccurrence {
                span: token.span,
                kind: *kind,
            })
        })
        .collect()
}

fn semantic_token_sort_index(kind: ql_analysis::SymbolKind) -> u32 {
    match kind {
        ql_analysis::SymbolKind::Import => 0,
        ql_analysis::SymbolKind::BuiltinType | ql_analysis::SymbolKind::TypeAlias => 1,
        ql_analysis::SymbolKind::Struct => 2,
        ql_analysis::SymbolKind::Enum => 3,
        ql_analysis::SymbolKind::Variant => 4,
        ql_analysis::SymbolKind::Trait => 5,
        ql_analysis::SymbolKind::Generic => 6,
        ql_analysis::SymbolKind::Parameter => 7,
        ql_analysis::SymbolKind::Local | ql_analysis::SymbolKind::SelfParameter => 8,
        ql_analysis::SymbolKind::Field => 9,
        ql_analysis::SymbolKind::Function
        | ql_analysis::SymbolKind::Const
        | ql_analysis::SymbolKind::Static => 10,
        ql_analysis::SymbolKind::Method => 11,
    }
}

fn sort_and_dedup_semantic_tokens(tokens: &mut Vec<SemanticTokenOccurrence>) {
    tokens.sort_by_key(|token| {
        (
            token.span.start,
            token.span.end,
            semantic_token_sort_index(token.kind),
        )
    });
    tokens.dedup_by(|left, right| left.span == right.span && left.kind == right.kind);
}

fn dependency_member_semantic_tokens_with_open_docs(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = lex(source)
        .0
        .iter()
        .filter(|token| token.kind == TokenKind::Ident)
        .filter_map(|token| {
            let position = span_to_range(source, token.span).start;
            let open_occurrence_span =
                dependency_occurrence_span_with_open_docs_at(source, package, open_docs, position)?;
            if open_occurrence_span != token.span {
                return None;
            }
            let open_target = dependency_definition_target_with_open_docs_at(
                source, analysis, package, open_docs, position,
            )?;
            let kind = match open_target.kind {
                ql_analysis::SymbolKind::Field => ql_analysis::SymbolKind::Field,
                ql_analysis::SymbolKind::Method => ql_analysis::SymbolKind::Method,
                _ => return None,
            };

            let disk_occurrence_span = dependency_occurrence_span_at(source, package, position);
            let disk_target = dependency_definition_target_at(source, analysis, package, position);
            let changed = disk_occurrence_span != Some(token.span)
                || match disk_target.as_ref() {
                    Some(target) => !same_dependency_definition_target(target, &open_target),
                    None => true,
                };
            changed.then_some(SemanticTokenOccurrence {
                span: token.span,
                kind,
            })
        })
        .collect::<Vec<_>>();
    sort_and_dedup_semantic_tokens(&mut tokens);
    tokens
}

pub(super) fn semantic_tokens_for_workspace_package_analysis_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> SemanticTokensResult {
    let tokens = workspace_package_semantic_token_occurrences_with_open_docs(
        uri, source, analysis, package, open_docs,
    );
    semantic_tokens_result_from_package_occurrences_with_lexical(source, tokens, package)
}

fn semantic_tokens_for_workspace_package_analysis_range_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    range: Range,
) -> SemanticTokensResult {
    let tokens = workspace_package_semantic_token_occurrences_with_open_docs(
        uri, source, analysis, package, open_docs,
    );
    semantic_tokens_result_from_package_occurrences_with_lexical_range(
        source, tokens, package, range,
    )
}

fn workspace_package_semantic_token_occurrences_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = analysis.semantic_tokens();
    let dependency_import_root_tokens =
        package.dependency_import_root_semantic_tokens_in_source(source);
    let dependency_member_tokens = dependency_member_semantic_tokens_with_open_docs(
        source,
        Some(analysis),
        package,
        open_docs,
    );
    let workspace_import_root_tokens = workspace_import_semantic_tokens_in_analysis_with_open_docs(
        uri, source, analysis, package, open_docs,
    );
    let overridden_import_spans = dependency_import_root_tokens
        .iter()
        .chain(workspace_import_root_tokens.iter())
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();
    let overridden_dependency_member_spans = dependency_member_tokens
        .iter()
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();

    tokens.retain(|token| {
        let span = (token.span.start, token.span.end);
        (token.kind != ql_analysis::SymbolKind::Import || !overridden_import_spans.contains(&span))
            && !overridden_dependency_member_spans.contains(&span)
    });
    tokens.extend(package.dependency_semantic_tokens_in_source(source));
    tokens.retain(|token| {
        !overridden_dependency_member_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(dependency_member_tokens);
    tokens.extend(dependency_import_root_tokens);
    tokens.extend(workspace_import_root_tokens);
    sort_and_dedup_semantic_tokens(&mut tokens);
    tokens
}

pub(super) fn semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> SemanticTokensResult {
    let tokens = workspace_dependency_fallback_semantic_token_occurrences_with_open_docs(
        uri, source, package, open_docs,
    );
    semantic_tokens_result_from_package_occurrences_with_lexical(source, tokens, package)
}

fn semantic_tokens_for_workspace_dependency_fallback_range_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    range: Range,
) -> SemanticTokensResult {
    let tokens = workspace_dependency_fallback_semantic_token_occurrences_with_open_docs(
        uri, source, package, open_docs,
    );
    semantic_tokens_result_from_package_occurrences_with_lexical_range(
        source, tokens, package, range,
    )
}

fn workspace_dependency_fallback_semantic_token_occurrences_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = package.dependency_fallback_semantic_tokens_in_source(source);
    let dependency_member_tokens =
        dependency_member_semantic_tokens_with_open_docs(source, None, package, open_docs);
    let overridden_dependency_member_spans = dependency_member_tokens
        .iter()
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();
    tokens.retain(|token| {
        !overridden_dependency_member_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(dependency_member_tokens);
    tokens.extend(
        workspace_import_semantic_tokens_in_broken_source_with_open_docs(
            uri, source, package, open_docs,
        ),
    );
    sort_and_dedup_semantic_tokens(&mut tokens);
    tokens
}
