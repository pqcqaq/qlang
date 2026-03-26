mod expr;
mod item;
mod pattern;
mod stmt;

use ql_ast::{Expr, ExprKind, Module, PackageDecl, Path, UseDecl, UseItem};
use ql_lexer::{Token, TokenKind, lex};
use ql_span::Span;

use crate::ParseError;

/// Parse a source file into the current AST module representation.
pub fn parse_source(source: &str) -> Result<Module, Vec<ParseError>> {
    let (tokens, lex_errors) = lex(source);
    if !lex_errors.is_empty() {
        return Err(lex_errors
            .into_iter()
            .map(|error| ParseError {
                message: error.message,
                span: error.span,
            })
            .collect());
    }

    Parser::new(tokens).parse_module()
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            idx: 0,
            errors: Vec::new(),
        }
    }

    fn parse_module(mut self) -> Result<Module, Vec<ParseError>> {
        let package = if self.at(TokenKind::Package) {
            match self.parse_package_decl() {
                Ok(package) => Some(package),
                Err(()) => return Err(self.errors),
            }
        } else {
            None
        };

        let mut uses = Vec::new();
        while self.at(TokenKind::Use) {
            match self.parse_use_decl() {
                Ok(use_decl) => uses.push(use_decl),
                Err(()) => return Err(self.errors),
            }
        }

        let mut items = Vec::new();
        while !self.at(TokenKind::Eof) {
            match self.parse_item() {
                Some(item) => items.push(item),
                None => self.synchronize_item(),
            }
        }

        if self.errors.is_empty() {
            Ok(Module {
                package,
                uses,
                items,
            })
        } else {
            Err(self.errors)
        }
    }

    fn parse_package_decl(&mut self) -> Result<PackageDecl, ()> {
        self.expect(TokenKind::Package, "expected `package` declaration")?;
        let path = self.parse_path()?;
        Ok(PackageDecl { path })
    }

    fn parse_use_decl(&mut self) -> Result<UseDecl, ()> {
        self.expect(TokenKind::Use, "expected `use` declaration")?;
        let prefix = self.parse_path()?;

        let group = if self.eat(TokenKind::Dot) {
            self.expect(TokenKind::LBrace, "expected `{` after `.` in grouped use")?;
            let mut items = Vec::new();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                let name = self.expect_ident_token("expected imported symbol name")?;
                let alias = if self.eat(TokenKind::As) {
                    Some(self.expect_ident_token("expected alias name after `as`")?)
                } else {
                    None
                };
                items.push(UseItem {
                    name: name.text,
                    name_span: name.span,
                    alias: alias.as_ref().map(|token| token.text.clone()),
                    alias_span: alias.as_ref().map(|token| token.span),
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "expected `}` to close grouped use")?;
            Some(items)
        } else {
            None
        };

        let alias = if self.eat(TokenKind::As) {
            Some(self.expect_ident_token("expected alias name after `as`")?)
        } else {
            None
        };

        Ok(UseDecl {
            prefix,
            group,
            alias: alias.as_ref().map(|token| token.text.clone()),
            alias_span: alias.as_ref().map(|token| token.span),
        })
    }

    fn parse_path(&mut self) -> Result<Path, ()> {
        let first = self.expect_ident_token("expected identifier path segment")?;
        let mut segments = vec![first.text];
        let mut segment_spans = vec![first.span];
        while self.at(TokenKind::Dot) && self.nth_kind(1) == TokenKind::Ident {
            self.bump();
            let segment = self.expect_ident_token("expected identifier after `.`")?;
            segments.push(segment.text);
            segment_spans.push(segment.span);
        }
        Ok(Path::with_spans(segments, segment_spans))
    }

    fn synchronize_item(&mut self) {
        while !self.at(TokenKind::Eof) {
            if matches!(
                self.current().kind,
                TokenKind::Pub
                    | TokenKind::Async
                    | TokenKind::Unsafe
                    | TokenKind::Fn
                    | TokenKind::Const
                    | TokenKind::Static
                    | TokenKind::Type
                    | TokenKind::Opaque
                    | TokenKind::Struct
                    | TokenKind::Data
                    | TokenKind::Enum
                    | TokenKind::Trait
                    | TokenKind::Impl
                    | TokenKind::Extend
                    | TokenKind::Extern
            ) {
                break;
            }
            self.bump();
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, ()> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            self.error_here(message);
            Err(())
        }
    }

    fn expect_ident_token(&mut self, message: &str) -> Result<Token, ()> {
        if self.at(TokenKind::Ident) {
            Ok(self.bump())
        } else {
            self.error_here(message);
            Err(())
        }
    }

    fn error_here(&mut self, message: &str) {
        self.errors.push(ParseError {
            message: message.into(),
            span: self.current().span,
        });
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn nth_kind(&self, offset: usize) -> TokenKind {
        self.tokens
            .get(self.idx + offset)
            .map(|token| token.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.idx.min(self.tokens.len().saturating_sub(1))]
    }

    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at(TokenKind::Eof) {
            self.idx += 1;
        }
        token
    }

    fn current_start(&self) -> usize {
        self.current().span.start
    }

    fn previous_end(&self) -> usize {
        self.tokens
            .get(self.idx.saturating_sub(1))
            .map(|token| token.span.end)
            .unwrap_or_else(|| self.current().span.start)
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.previous_end())
    }
}

fn expr_to_path(expr: &Expr) -> Option<Path> {
    match &expr.kind {
        ExprKind::Name(name) => Some(Path::with_spans(vec![name.clone()], vec![expr.span])),
        ExprKind::Member {
            object,
            field,
            field_span,
        } => {
            let mut path = expr_to_path(object)?;
            path.segments.push(field.clone());
            path.segment_spans.push(*field_span);
            Some(path)
        }
        _ => None,
    }
}

fn is_binding_name(name: &str) -> bool {
    name.chars()
        .next()
        .map(|ch| ch == '_' || ch.is_lowercase())
        .unwrap_or(false)
}
