use ql_span::Span;

/// A syntax error produced while lexing or parsing a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}
