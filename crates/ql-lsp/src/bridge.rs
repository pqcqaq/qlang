use ql_analysis::{Analysis, HoverInfo, SymbolKind};
use ql_diagnostics::{
    Diagnostic as CompilerDiagnostic, DiagnosticSeverity as CompilerSeverity, Label,
};
use ql_span::Span;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, GotoDefinitionResponse, Hover,
    HoverContents, Location, MarkupContent, MarkupKind, Position, Range, Url,
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
        SymbolKind::Trait => "trait",
        SymbolKind::TypeAlias => "type alias",
        SymbolKind::Local => "local",
        SymbolKind::Parameter => "parameter",
        SymbolKind::Generic => "generic",
        SymbolKind::SelfParameter => "receiver",
        SymbolKind::BuiltinType => "builtin type",
        SymbolKind::Import => "import",
    }
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
