use ql_span::Span;

/// A lexed token with its original text and source span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

/// A recoverable lexical error produced during tokenization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

/// Token categories recognized by the current hand-written lexer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Eof,
    Ident,
    Int,
    String,
    FormatString,
    Package,
    Use,
    Pub,
    Const,
    Static,
    Let,
    Var,
    Fn,
    Async,
    Await,
    Spawn,
    Defer,
    Return,
    Break,
    Continue,
    If,
    Else,
    Match,
    For,
    While,
    Loop,
    In,
    Where,
    Struct,
    Data,
    Enum,
    Trait,
    Impl,
    Extend,
    Type,
    Opaque,
    Extern,
    Unsafe,
    Is,
    As,
    Satisfies,
    NoneKw,
    TrueKw,
    FalseKw,
    SelfKw,
    MoveKw,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Dot,
    DotDot,
    Colon,
    Semi,
    At,
    Arrow,
    FatArrow,
    Question,
    Eq,
    EqEq,
    BangEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Underscore,
}

pub fn is_keyword(text: &str) -> bool {
    keyword_kind(text).is_some()
}

/// Lex a source file into tokens and recoverable lexical errors.
pub fn lex(source: &str) -> (Vec<Token>, Vec<LexError>) {
    let mut lexer = Lexer::new(source);
    lexer.lex_all()
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    idx: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().collect(),
            idx: 0,
        }
    }

    fn lex_all(&mut self) -> (Vec<Token>, Vec<LexError>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        while let Some((start, ch)) = self.peek() {
            if ch.is_whitespace() {
                self.bump();
                continue;
            }

            if ch == '/' {
                if self.peek_next_char() == Some('/') {
                    self.bump();
                    self.bump();
                    self.consume_until(|c| c == '\n');
                    continue;
                }

                if self.peek_next_char() == Some('*') {
                    self.bump();
                    self.bump();

                    let mut terminated = false;
                    while self.peek().is_some() {
                        if self.peek_char() == Some('*') && self.peek_next_char() == Some('/') {
                            self.bump();
                            self.bump();
                            terminated = true;
                            break;
                        }
                        self.bump();
                    }

                    if !terminated {
                        errors.push(LexError {
                            message: "unterminated block comment".into(),
                            span: Span::new(start, self.current_offset()),
                        });
                    }
                    continue;
                }
            }

            if ch == 'f' && self.peek_next_char() == Some('"') {
                match self.lex_string(true) {
                    Ok(token) => tokens.push(token),
                    Err(error) => errors.push(error),
                }
                continue;
            }

            if ch == '"' {
                match self.lex_string(false) {
                    Ok(token) => tokens.push(token),
                    Err(error) => errors.push(error),
                }
                continue;
            }

            if ch.is_ascii_digit() {
                match self.lex_number() {
                    Ok(token) => tokens.push(token),
                    Err(error) => errors.push(error),
                }
                continue;
            }

            if ch == '_' {
                if self.peek_next_char().is_some_and(is_ident_continue) {
                    tokens.push(self.lex_ident_or_keyword());
                } else {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Underscore,
                        text: "_".into(),
                        span: Span::new(start, self.current_offset()),
                    });
                }
                continue;
            }

            if ch == '`' {
                match self.lex_escaped_ident() {
                    Ok(token) => tokens.push(token),
                    Err(error) => errors.push(error),
                }
                continue;
            }

            if is_ident_start(ch) {
                tokens.push(self.lex_ident_or_keyword());
                continue;
            }

            let token = match ch {
                '(' => Some(self.single(TokenKind::LParen)),
                ')' => Some(self.single(TokenKind::RParen)),
                '{' => Some(self.single(TokenKind::LBrace)),
                '}' => Some(self.single(TokenKind::RBrace)),
                '[' => Some(self.single(TokenKind::LBracket)),
                ']' => Some(self.single(TokenKind::RBracket)),
                ',' => Some(self.single(TokenKind::Comma)),
                ':' => Some(self.single(TokenKind::Colon)),
                ';' => Some(self.single(TokenKind::Semi)),
                '?' => Some(self.single(TokenKind::Question)),
                '@' => Some(self.single(TokenKind::At)),
                '+' => Some(self.single(TokenKind::Plus)),
                '*' => Some(self.single(TokenKind::Star)),
                '/' => Some(self.single(TokenKind::Slash)),
                '%' => Some(self.single(TokenKind::Percent)),
                '.' => {
                    self.bump();
                    if self.peek_char() == Some('.') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::DotDot,
                            text: "..".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else {
                        Some(Token {
                            kind: TokenKind::Dot,
                            text: ".".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    }
                }
                '-' => {
                    self.bump();
                    if self.peek_char() == Some('>') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::Arrow,
                            text: "->".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else {
                        Some(Token {
                            kind: TokenKind::Minus,
                            text: "-".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    }
                }
                '=' => {
                    self.bump();
                    if self.peek_char() == Some('>') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::FatArrow,
                            text: "=>".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else if self.peek_char() == Some('=') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::EqEq,
                            text: "==".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else {
                        Some(Token {
                            kind: TokenKind::Eq,
                            text: "=".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    }
                }
                '!' => {
                    self.bump();
                    if self.peek_char() == Some('=') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::BangEq,
                            text: "!=".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else {
                        errors.push(LexError {
                            message: "unexpected `!`; only `!=` is currently supported".into(),
                            span: Span::new(start, self.current_offset()),
                        });
                        None
                    }
                }
                '<' => {
                    self.bump();
                    if self.peek_char() == Some('=') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::LtEq,
                            text: "<=".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else {
                        Some(Token {
                            kind: TokenKind::Lt,
                            text: "<".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    }
                }
                '>' => {
                    self.bump();
                    if self.peek_char() == Some('=') {
                        self.bump();
                        Some(Token {
                            kind: TokenKind::GtEq,
                            text: ">=".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    } else {
                        Some(Token {
                            kind: TokenKind::Gt,
                            text: ">".into(),
                            span: Span::new(start, self.current_offset()),
                        })
                    }
                }
                _ => None,
            };

            if let Some(token) = token {
                tokens.push(token);
            } else {
                self.bump();
                errors.push(LexError {
                    message: format!("unexpected character `{ch}`"),
                    span: Span::new(start, self.current_offset()),
                });
            }
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            text: String::new(),
            span: Span::new(self.current_offset(), self.current_offset()),
        });

        (tokens, errors)
    }

    fn lex_string(&mut self, is_format: bool) -> Result<Token, LexError> {
        let start = self.current_offset();
        if is_format {
            self.bump();
        }
        self.bump();

        let content_start = self.current_offset();
        while let Some((_, ch)) = self.peek() {
            if ch == '"' {
                let end = self.current_offset();
                let text = self.source[content_start..end].to_string();
                self.bump();
                return Ok(Token {
                    kind: if is_format {
                        TokenKind::FormatString
                    } else {
                        TokenKind::String
                    },
                    text,
                    span: Span::new(start, self.current_offset()),
                });
            }

            if ch == '\\' {
                self.bump();
                if self.peek().is_some() {
                    self.bump();
                }
                continue;
            }

            self.bump();
        }

        Err(LexError {
            message: "unterminated string literal".into(),
            span: Span::new(start, self.current_offset()),
        })
    }

    fn lex_escaped_ident(&mut self) -> Result<Token, LexError> {
        let start = self.current_offset();
        self.bump();

        let ident_start = self.current_offset();
        if !self.peek_char().is_some_and(is_ident_start) {
            return Err(LexError {
                message: "escaped identifier must start with a valid identifier character".into(),
                span: Span::new(start, self.current_offset()),
            });
        }

        self.bump();
        self.consume_while(is_ident_continue);
        let ident_end = self.current_offset();

        if self.peek_char() != Some('`') {
            self.consume_until(|ch| ch == '`' || ch.is_whitespace());
            if self.peek_char() == Some('`') {
                self.bump();
            }
            return Err(LexError {
                message: "unterminated escaped identifier".into(),
                span: Span::new(start, self.current_offset()),
            });
        }

        self.bump();
        Ok(Token {
            kind: TokenKind::Ident,
            text: self.source[ident_start..ident_end].to_string(),
            span: Span::new(start, self.current_offset()),
        })
    }

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let start = self.current_offset();
        self.bump();

        if self.source.as_bytes()[start] == b'0' {
            match self.peek_char() {
                Some('x') | Some('X') => {
                    self.bump();
                    if !self.consume_number_digits(|ch| ch.is_ascii_hexdigit()) {
                        return Err(LexError {
                            message: "expected hexadecimal digits after `0x`".into(),
                            span: Span::new(start, self.current_offset()),
                        });
                    }
                }
                Some('b') | Some('B') => {
                    self.bump();
                    if !self.consume_number_digits(|ch| matches!(ch, '0' | '1')) {
                        return Err(LexError {
                            message: "expected binary digits after `0b`".into(),
                            span: Span::new(start, self.current_offset()),
                        });
                    }
                }
                Some('o') | Some('O') => {
                    self.bump();
                    if !self.consume_number_digits(|ch| matches!(ch, '0'..='7')) {
                        return Err(LexError {
                            message: "expected octal digits after `0o`".into(),
                            span: Span::new(start, self.current_offset()),
                        });
                    }
                }
                _ => {
                    self.consume_number_digits(|ch| ch.is_ascii_digit());
                }
            }
        } else {
            self.consume_number_digits(|ch| ch.is_ascii_digit());
        }

        let literal_end = self.current_offset();
        if self.peek_char().is_some_and(is_ident_start) {
            self.consume_while(is_ident_continue);
            return Err(LexError {
                message: "invalid numeric literal suffix".into(),
                span: Span::new(start, self.current_offset()),
            });
        }

        Ok(Token {
            kind: TokenKind::Int,
            text: self.source[start..literal_end].to_string(),
            span: Span::new(start, literal_end),
        })
    }

    fn lex_ident_or_keyword(&mut self) -> Token {
        let start = self.current_offset();
        self.bump();
        self.consume_while(is_ident_continue);
        let text = &self.source[start..self.current_offset()];
        let kind = keyword_kind(text).unwrap_or(TokenKind::Ident);
        Token {
            kind,
            text: text.to_string(),
            span: Span::new(start, self.current_offset()),
        }
    }

    fn single(&mut self, kind: TokenKind) -> Token {
        let start = self.current_offset();
        self.bump();
        Token {
            kind,
            text: self.source[start..self.current_offset()].to_string(),
            span: Span::new(start, self.current_offset()),
        }
    }

    fn consume_until(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some((_, ch)) = self.peek() {
            if predicate(ch) {
                break;
            }
            self.bump();
        }
    }

    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some((_, ch)) = self.peek() {
            if !predicate(ch) {
                break;
            }
            self.bump();
        }
    }

    fn consume_number_digits(&mut self, predicate: impl Fn(char) -> bool) -> bool {
        let mut consumed_digit = false;
        while let Some((_, ch)) = self.peek() {
            if predicate(ch) {
                consumed_digit = true;
                self.bump();
            } else if ch == '_' {
                self.bump();
            } else {
                break;
            }
        }
        consumed_digit
    }

    fn bump(&mut self) -> Option<(usize, char)> {
        let next = self.chars.get(self.idx).copied();
        self.idx += usize::from(next.is_some());
        next
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.idx).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.peek().map(|(_, ch)| ch)
    }

    fn peek_next_char(&self) -> Option<char> {
        self.chars.get(self.idx + 1).map(|(_, ch)| *ch)
    }

    fn current_offset(&self) -> usize {
        self.peek()
            .map(|(offset, _)| offset)
            .unwrap_or_else(|| self.source.len())
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn keyword_kind(text: &str) -> Option<TokenKind> {
    Some(match text {
        "package" => TokenKind::Package,
        "use" => TokenKind::Use,
        "pub" => TokenKind::Pub,
        "const" => TokenKind::Const,
        "static" => TokenKind::Static,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        "fn" => TokenKind::Fn,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "spawn" => TokenKind::Spawn,
        "defer" => TokenKind::Defer,
        "return" => TokenKind::Return,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "match" => TokenKind::Match,
        "for" => TokenKind::For,
        "while" => TokenKind::While,
        "loop" => TokenKind::Loop,
        "in" => TokenKind::In,
        "where" => TokenKind::Where,
        "struct" => TokenKind::Struct,
        "data" => TokenKind::Data,
        "enum" => TokenKind::Enum,
        "trait" => TokenKind::Trait,
        "impl" => TokenKind::Impl,
        "extend" => TokenKind::Extend,
        "type" => TokenKind::Type,
        "opaque" => TokenKind::Opaque,
        "extern" => TokenKind::Extern,
        "unsafe" => TokenKind::Unsafe,
        "is" => TokenKind::Is,
        "as" => TokenKind::As,
        "satisfies" => TokenKind::Satisfies,
        "none" => TokenKind::NoneKw,
        "true" => TokenKind::TrueKw,
        "false" => TokenKind::FalseKw,
        "self" => TokenKind::SelfKw,
        "move" => TokenKind::MoveKw,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, lex};

    fn token_kinds(source: &str) -> Vec<TokenKind> {
        let (tokens, errors) = lex(source);
        assert!(errors.is_empty(), "unexpected lex errors: {errors:?}");
        tokens.into_iter().map(|token| token.kind).collect()
    }

    #[test]
    fn lexes_underscore_prefixed_identifiers() {
        let (tokens, errors) = lex("let _value = 1");

        assert!(errors.is_empty(), "unexpected lex errors: {errors:?}");
        assert_eq!(tokens[1].kind, TokenKind::Ident);
        assert_eq!(tokens[1].text, "_value");
    }

    #[test]
    fn lexes_escaped_identifiers_as_plain_idents() {
        let (tokens, errors) = lex("let `type` = 1");

        assert!(errors.is_empty(), "unexpected lex errors: {errors:?}");
        assert_eq!(tokens[1].kind, TokenKind::Ident);
        assert_eq!(tokens[1].text, "type");
    }

    #[test]
    fn rejects_numeric_literals_with_identifier_suffixes() {
        let (tokens, errors) = lex("let value = 1abc");

        assert_eq!(
            token_kinds("let _ = 0xff"),
            vec![
                TokenKind::Let,
                TokenKind::Underscore,
                TokenKind::Eq,
                TokenKind::Int,
                TokenKind::Eof
            ]
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "invalid numeric literal suffix");
        assert_eq!(tokens.last().map(|token| token.kind), Some(TokenKind::Eof));
    }
}
