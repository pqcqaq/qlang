use ql_fmt::format_source;
use ql_span::Span;
use tower_lsp::lsp_types::{Position, Range, TextEdit};

use crate::bridge::{position_to_offset, span_to_range};

#[derive(Debug, PartialEq, Eq)]
struct FormattingEdit {
    span: Span,
    replacement: String,
}

pub(super) fn document_formatting_edits(
    source: &str,
) -> std::result::Result<Vec<TextEdit>, String> {
    let formatted = format_source_for_lsp(source)?;
    if formatted == source {
        return Ok(Vec::new());
    }

    Ok(vec![TextEdit::new(
        span_to_range(source, Span::new(0, source.len())),
        formatted,
    )])
}

pub(super) fn range_formatting_edits(
    source: &str,
    range: Range,
) -> std::result::Result<Vec<TextEdit>, String> {
    let formatted = format_source_for_lsp(source)?;
    let requested = range_offsets(source, range)?;
    Ok(line_formatting_edits(source, &formatted)
        .into_iter()
        .filter(|edit| edit.span.start >= requested.start && edit.span.end <= requested.end)
        .map(|edit| TextEdit::new(span_to_range(source, edit.span), edit.replacement))
        .collect())
}

pub(super) fn on_type_formatting_edits(
    source: &str,
    position: Position,
    ch: &str,
) -> std::result::Result<Vec<TextEdit>, String> {
    if !matches!(ch, "\n" | "}" | ";" | ",") {
        return Ok(Vec::new());
    }
    position_to_offset(source, position).ok_or_else(|| {
        "qlang: formatting skipped because the trigger position is invalid".to_owned()
    })?;
    let formatted = format_source_for_lsp(source)?;
    Ok(line_formatting_edits(source, &formatted)
        .into_iter()
        .filter(|edit| {
            let range = span_to_range(source, edit.span);
            range.start.line == position.line && range.end.line == position.line
        })
        .map(|edit| TextEdit::new(span_to_range(source, edit.span), edit.replacement))
        .collect())
}

fn format_source_for_lsp(source: &str) -> std::result::Result<String, String> {
    format_source(source).map_err(|errors| {
        let Some(error) = errors.first() else {
            return "qlang: document formatting skipped because the document has parse errors"
                .to_owned();
        };
        let range = span_to_range(source, error.span);
        format!(
            "qlang: document formatting skipped because the document has parse errors at {}:{}: {}",
            range.start.line + 1,
            range.start.character + 1,
            error.message
        )
    })
}

fn range_offsets(source: &str, range: Range) -> std::result::Result<Span, String> {
    let start = position_to_offset(source, range.start)
        .ok_or_else(|| "qlang: formatting skipped because the range start is invalid".to_owned())?;
    let end = position_to_offset(source, range.end)
        .ok_or_else(|| "qlang: formatting skipped because the range end is invalid".to_owned())?;
    Ok(Span::new(start.min(end), start.max(end)))
}

fn line_formatting_edits(source: &str, formatted: &str) -> Vec<FormattingEdit> {
    let source_lines = line_content_spans(source);
    let formatted_lines = line_content_spans(formatted);
    if source_lines.len() != formatted_lines.len() {
        return Vec::new();
    }

    source_lines
        .into_iter()
        .zip(formatted_lines)
        .filter_map(|((source_span, source_line), (_, formatted_line))| {
            line_formatting_edit(source_span, source_line, formatted_line)
        })
        .collect()
}

fn line_formatting_edit(
    source_span: Span,
    source_line: &str,
    formatted_line: &str,
) -> Option<FormattingEdit> {
    if source_line == formatted_line {
        return None;
    }
    let prefix = common_prefix_len(source_line, formatted_line);
    let (source_end, formatted_end) = common_suffix_starts(source_line, formatted_line, prefix);
    Some(FormattingEdit {
        span: Span::new(source_span.start + prefix, source_span.start + source_end),
        replacement: formatted_line[prefix..formatted_end].to_owned(),
    })
}

fn line_content_spans(source: &str) -> Vec<(Span, &str)> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    for (offset, ch) in source.char_indices() {
        if ch != '\n' {
            continue;
        }
        let content_end = if offset > line_start && source.as_bytes()[offset - 1] == b'\r' {
            offset - 1
        } else {
            offset
        };
        lines.push((
            Span::new(line_start, content_end),
            &source[line_start..content_end],
        ));
        line_start = offset + ch.len_utf8();
    }
    if line_start < source.len() || source.ends_with('\n') {
        lines.push((
            Span::new(line_start, source.len()),
            &source[line_start..source.len()],
        ));
    }
    lines
}

fn common_prefix_len(left: &str, right: &str) -> usize {
    let mut matched = 0usize;
    for ((left_index, left_char), (right_index, right_char)) in
        left.char_indices().zip(right.char_indices())
    {
        if left_char != right_char {
            return left_index.min(right_index);
        }
        matched = left_index + left_char.len_utf8();
    }
    matched
}

fn common_suffix_starts(left: &str, right: &str, prefix: usize) -> (usize, usize) {
    let mut left_end = left.len();
    let mut right_end = right.len();
    let mut left_chars = left.char_indices().rev();
    let mut right_chars = right.char_indices().rev();
    while let (Some((left_index, left_char)), Some((right_index, right_char))) =
        (left_chars.next(), right_chars.next())
    {
        if left_index < prefix || right_index < prefix || left_char != right_char {
            break;
        }
        left_end = left_index;
        right_end = right_index;
    }
    (left_end, right_end)
}
