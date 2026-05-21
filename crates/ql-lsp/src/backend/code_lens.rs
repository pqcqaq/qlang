use ql_analysis::Analysis;
use serde_json::json;
use tower_lsp::lsp_types::request::GotoImplementationResponse;
use tower_lsp::lsp_types::{
    CodeLens, Command, DocumentSymbol, DocumentSymbolResponse, Location, Position, Range, Url,
};

use crate::bridge::{
    document_symbols_for_analysis, implementation_for_analysis, references_for_analysis,
    references_for_package_analysis,
};

use super::{
    OpenDocuments, fallback_implementation_for_analysis,
    workspace_source_implementation_with_open_docs,
    workspace_source_references_for_root_symbol_with_open_docs,
};

pub(super) fn code_lenses_for_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
) -> Vec<CodeLens> {
    let mut lenses = Vec::new();
    for (range, position) in code_lens_targets_for_analysis(source, analysis) {
        if let Some(locations) = references_for_analysis(uri, source, analysis, position, false)
            && !locations.is_empty()
        {
            lenses.push(CodeLens {
                range,
                command: Some(show_locations_command(
                    location_count_title(locations.len(), "reference", "references"),
                    uri,
                    position,
                    &locations,
                )),
                data: None,
            });
        }

        if let Some(implementation) = implementation_for_analysis(uri, source, analysis, position) {
            let locations = locations_from_goto_response(implementation);
            if !locations.is_empty() {
                lenses.push(CodeLens {
                    range,
                    command: Some(show_locations_command(
                        location_count_title(locations.len(), "implementation", "implementations"),
                        uri,
                        position,
                        &locations,
                    )),
                    data: None,
                });
            }
        }
    }
    lenses
}

pub(super) fn code_lenses_for_workspace_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<CodeLens> {
    let mut lenses = Vec::new();
    for (range, position) in code_lens_targets_for_analysis(source, analysis) {
        let references = workspace_source_references_for_root_symbol_with_open_docs(
            uri, source, analysis, package, open_docs, position, false,
        )
        .or_else(|| {
            references_for_package_analysis(uri, source, analysis, package, position, false)
        });
        if let Some(locations) = references
            && !locations.is_empty()
        {
            lenses.push(CodeLens {
                range,
                command: Some(show_locations_command(
                    location_count_title(locations.len(), "reference", "references"),
                    uri,
                    position,
                    &locations,
                )),
                data: None,
            });
        }

        let implementation = workspace_source_implementation_with_open_docs(
            uri,
            source,
            Some(analysis),
            package,
            open_docs,
            position,
        )
        .or_else(|| fallback_implementation_for_analysis(uri, source, Some(analysis), position));
        if let Some(implementation) = implementation {
            let locations = locations_from_goto_response(implementation);
            if !locations.is_empty() {
                lenses.push(CodeLens {
                    range,
                    command: Some(show_locations_command(
                        location_count_title(locations.len(), "implementation", "implementations"),
                        uri,
                        position,
                        &locations,
                    )),
                    data: None,
                });
            }
        }
    }
    lenses
}

fn code_lens_targets_for_analysis(source: &str, analysis: &Analysis) -> Vec<(Range, Position)> {
    match document_symbols_for_analysis(source, analysis) {
        DocumentSymbolResponse::Nested(symbols) => {
            let mut targets = Vec::new();
            collect_code_lens_targets_from_symbols(&symbols, &mut targets);
            targets
        }
        DocumentSymbolResponse::Flat(symbols) => symbols
            .into_iter()
            .map(|symbol| (symbol.location.range, symbol.location.range.start))
            .collect(),
    }
}

fn collect_code_lens_targets_from_symbols(
    symbols: &[DocumentSymbol],
    targets: &mut Vec<(Range, Position)>,
) {
    for symbol in symbols {
        targets.push((symbol.range, symbol.selection_range.start));
        if let Some(children) = symbol.children.as_ref() {
            collect_code_lens_targets_from_symbols(children, targets);
        }
    }
}

fn locations_from_goto_response(response: GotoImplementationResponse) -> Vec<Location> {
    match response {
        GotoImplementationResponse::Scalar(location) => vec![location],
        GotoImplementationResponse::Array(locations) => locations,
        GotoImplementationResponse::Link(links) => links
            .into_iter()
            .map(|link| Location::new(link.target_uri, link.target_range))
            .collect(),
    }
}

fn location_count_title(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn show_locations_command(
    title: String,
    uri: &Url,
    position: Position,
    locations: &[Location],
) -> Command {
    Command {
        title,
        command: "editor.action.showReferences".to_owned(),
        arguments: Some(vec![json!(uri), json!(position), json!(locations)]),
    }
}
