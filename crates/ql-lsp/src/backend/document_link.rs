use ql_analysis::{DependencyInterface, PackageAnalysis};
use ql_lexer::{Token, TokenKind, lex};
use ql_span::Span;
use tower_lsp::lsp_types::{DocumentLink, Url};

use crate::bridge::span_to_range;

pub(super) fn document_links_for_package_imports(
    source: &str,
    package: &PackageAnalysis,
) -> Option<Vec<DocumentLink>> {
    let (tokens, _) = lex(source);
    let mut links = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.kind == TokenKind::Use)
        .filter_map(|(index, _)| document_link_for_use_token(source, package, &tokens, index))
        .collect::<Vec<_>>();
    links.sort_by_key(|link| {
        (
            link.range.start.line,
            link.range.start.character,
            link.range.end.line,
            link.range.end.character,
        )
    });
    links.dedup_by_key(|link| (link.range, link.target.clone()));
    (!links.is_empty()).then_some(links)
}

fn document_link_for_use_token(
    source: &str,
    package: &PackageAnalysis,
    tokens: &[Token],
    use_index: usize,
) -> Option<DocumentLink> {
    let use_token = tokens.get(use_index)?;
    let line_end = source[use_token.span.end..]
        .find('\n')
        .map(|relative| use_token.span.end + relative)
        .unwrap_or(source.len());
    let mut path_start = None;
    let mut path_end = None;
    let mut segments = Vec::<String>::new();

    for token in tokens
        .iter()
        .skip(use_index + 1)
        .take_while(|token| token.span.start < line_end)
    {
        match token.kind {
            TokenKind::Ident => {
                path_start.get_or_insert(token.span.start);
                path_end = Some(token.span.end);
                segments.push(token.text.clone());
            }
            TokenKind::Dot => {
                if path_start.is_none() {
                    return None;
                }
            }
            TokenKind::As => break,
            _ => break,
        }
    }

    let path_start = path_start?;
    let path_end = path_end?;
    let dependency = document_link_dependency_for_import_segments(package, &segments)?;
    let target = Url::from_file_path(dependency.interface_path()).ok()?;
    Some(DocumentLink {
        range: span_to_range(source, Span::new(path_start, path_end)),
        target: Some(target),
        tooltip: Some(format!(
            "Open dependency interface `{}`",
            dependency.artifact().package_name
        )),
        data: None,
    })
}

fn document_link_dependency_for_import_segments<'a>(
    package: &'a PackageAnalysis,
    segments: &[String],
) -> Option<&'a DependencyInterface> {
    let imported_name = segments.last()?;
    let mut matches = package
        .dependencies()
        .iter()
        .filter(|dependency| {
            let package_segments = dependency
                .artifact()
                .package_name
                .split('.')
                .collect::<Vec<_>>();
            segments.len() > package_segments.len()
                && segments
                    .iter()
                    .take(package_segments.len())
                    .map(String::as_str)
                    .eq(package_segments.iter().copied())
                && !dependency.symbols_named(imported_name).is_empty()
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|dependency| dependency.artifact().package_name.len());
    matches.pop()
}
