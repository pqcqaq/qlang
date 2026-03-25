use ql_ast::{Pattern, PatternField};
use ql_lexer::TokenKind;

use super::{Parser, is_binding_name};

impl Parser {
    pub(super) fn parse_pattern(&mut self) -> Result<Pattern, ()> {
        match self.current().kind {
            TokenKind::Int => return Ok(Pattern::Integer(self.bump().text)),
            TokenKind::String => return Ok(Pattern::String(self.bump().text)),
            TokenKind::TrueKw => {
                self.bump();
                return Ok(Pattern::Bool(true));
            }
            TokenKind::FalseKw => {
                self.bump();
                return Ok(Pattern::Bool(false));
            }
            TokenKind::NoneKw => {
                self.bump();
                return Ok(Pattern::NoneLiteral);
            }
            _ => {}
        }

        if self.eat(TokenKind::Underscore) {
            return Ok(Pattern::Wildcard);
        }

        if self.eat(TokenKind::LParen) {
            let mut patterns = Vec::new();
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                patterns.push(self.parse_pattern()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "expected `)` after tuple pattern")?;
            return Ok(Pattern::Tuple(patterns));
        }

        let path = self.parse_path()?;
        if self.eat(TokenKind::LParen) {
            let mut items = Vec::new();
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                items.push(self.parse_pattern()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "expected `)` after tuple-struct pattern")?;
            return Ok(Pattern::TupleStruct { path, items });
        }

        if self.eat(TokenKind::LBrace) {
            let mut fields = Vec::new();
            let mut has_rest = false;
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                if self.eat(TokenKind::DotDot) {
                    has_rest = true;
                    self.eat(TokenKind::Comma);
                    break;
                }
                let name = self.expect_ident("expected pattern field")?;
                let pattern = if self.eat(TokenKind::Colon) {
                    Some(Box::new(self.parse_pattern()?))
                } else {
                    None
                };
                fields.push(PatternField { name, pattern });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "expected `}` after struct pattern")?;
            return Ok(Pattern::Struct {
                path,
                fields,
                has_rest,
            });
        }

        if path.segments.len() == 1 && is_binding_name(&path.segments[0]) {
            return Ok(Pattern::Name(path.segments.into_iter().next().unwrap()));
        }

        Ok(Pattern::Path(path))
    }
}
