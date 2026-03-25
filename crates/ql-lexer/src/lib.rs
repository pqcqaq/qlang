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
                tokens.push(self.lex_number());
                continue;
            }

            if ch == '_' {
                self.bump();
                tokens.push(Token {
                    kind: TokenKind::Underscore,
                    text: "_".into(),
                    span: Span::new(start, self.current_offset()),
                });
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

    fn lex_number(&mut self) -> Token {
        let start = self.current_offset();
        self.bump();
        self.consume_while(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        Token {
            kind: TokenKind::Int,
            text: self.source[start..self.current_offset()].to_string(),
            span: Span::new(start, self.current_offset()),
        }
    }

    fn lex_ident_or_keyword(&mut self) -> Token {
        let start = self.current_offset();
        self.bump();
        self.consume_while(is_ident_continue);
        let text = &self.source[start..self.current_offset()];
        let kind = match text {
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
            _ => TokenKind::Ident,
        };
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
