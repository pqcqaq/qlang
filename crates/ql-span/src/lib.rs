use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];

        for (offset, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(offset + ch.len_utf8());
            }
        }

        Self { line_starts }
    }

    pub fn line_col(&self, offset: usize) -> LineColumn {
        let idx = self
            .line_starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[idx];

        LineColumn {
            line: idx + 1,
            column: offset.saturating_sub(line_start) + 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub span: Span,
    pub start: LineColumn,
    pub end: LineColumn,
}

pub fn locate(source: &str, span: Span) -> SourceLocation {
    let index = LineIndex::new(source);

    SourceLocation {
        span,
        start: index.line_col(span.start),
        end: index.line_col(span.end),
    }
}

pub fn slice_for_line(source: &str, line: usize) -> Option<&str> {
    source.lines().nth(line.saturating_sub(1))
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
