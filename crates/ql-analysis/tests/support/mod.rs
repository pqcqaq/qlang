use ql_analysis::Analysis;
use ql_span::Span;

pub fn analyzed(source: &str) -> Analysis {
    ql_analysis::analyze_source(source).expect("source should analyze")
}

pub fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    nth_span(source, needle, occurrence).start
}

pub fn nth_span(source: &str, needle: &str, occurrence: usize) -> Span {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, matched)| Span::new(start, start + matched.len()))
        .expect("needle occurrence should exist")
}
