use ql_ast::{BinaryOp, CallArg, Expr, MatchArm, StructLiteralField};
use ql_lexer::TokenKind;

use super::{Parser, expr_to_path};

impl Parser {
    pub(super) fn parse_expr(&mut self) -> Result<Expr, ()> {
        if self.at(TokenKind::If) {
            return self.parse_if_expr();
        }

        if self.at(TokenKind::Match) {
            return self.parse_match_expr();
        }

        self.parse_binary_expr(0)
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ()> {
        self.expect(TokenKind::If, "expected `if`")?;
        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.eat(TokenKind::Else) {
            if self.at(TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else if self.at(TokenKind::LBrace) {
                Some(Box::new(Expr::Block(self.parse_block()?)))
            } else {
                self.error_here("expected `if` or block after `else`");
                return Err(());
            }
        } else {
            None
        };

        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch,
            else_branch,
        })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, ()> {
        self.expect(TokenKind::Match, "expected `match`")?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::LBrace, "expected `{` after match value")?;
        let mut arms = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let pattern = self.parse_pattern()?;
            let guard = if self.eat(TokenKind::If) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(TokenKind::FatArrow, "expected `=>` in match arm")?;
            let body = if self.at(TokenKind::LBrace) {
                Expr::Block(self.parse_block()?)
            } else {
                self.parse_expr()?
            };
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            self.eat(TokenKind::Comma);
        }

        self.expect(TokenKind::RBrace, "expected `}` after match arms")?;
        Ok(Expr::Match {
            value: Box::new(value),
            arms,
        })
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Result<Expr, ()> {
        let mut left = self.parse_prefix_expr()?;

        loop {
            let (op, prec, right_assoc) = match self.current().kind {
                TokenKind::Eq => (BinaryOp::Assign, 1, true),
                TokenKind::EqEq => (BinaryOp::EqEq, 2, false),
                TokenKind::BangEq => (BinaryOp::BangEq, 2, false),
                TokenKind::Gt => (BinaryOp::Gt, 3, false),
                TokenKind::GtEq => (BinaryOp::GtEq, 3, false),
                TokenKind::Lt => (BinaryOp::Lt, 3, false),
                TokenKind::LtEq => (BinaryOp::LtEq, 3, false),
                TokenKind::Plus => (BinaryOp::Add, 4, false),
                TokenKind::Minus => (BinaryOp::Sub, 4, false),
                TokenKind::Star => (BinaryOp::Mul, 5, false),
                TokenKind::Slash => (BinaryOp::Div, 5, false),
                TokenKind::Percent => (BinaryOp::Rem, 5, false),
                _ => break,
            };

            if prec < min_prec {
                break;
            }

            self.bump();
            let next_min_prec = if right_assoc { prec } else { prec + 1 };
            let right = self.parse_binary_expr(next_min_prec)?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_prefix_expr(&mut self) -> Result<Expr, ()> {
        if self.at(TokenKind::MoveKw) && self.is_closure_start(self.idx) {
            return self.parse_closure();
        }

        match self.current().kind {
            TokenKind::Await => {
                self.bump();
                let expr = self.parse_prefix_expr()?;
                Ok(Expr::Unary {
                    op: ql_ast::UnaryOp::Await,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Spawn => {
                self.bump();
                let expr = self.parse_prefix_expr()?;
                Ok(Expr::Unary {
                    op: ql_ast::UnaryOp::Spawn,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Minus => {
                self.bump();
                let expr = self.parse_prefix_expr()?;
                Ok(Expr::Unary {
                    op: ql_ast::UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            TokenKind::LParen if self.is_closure_start(self.idx) => self.parse_closure(),
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_closure(&mut self) -> Result<Expr, ()> {
        let is_move = self.eat(TokenKind::MoveKw);
        self.expect(
            TokenKind::LParen,
            "expected `(` to start closure parameters",
        )?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            params.push(self.expect_ident("expected closure parameter name")?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "expected `)` after closure parameters")?;
        self.expect(
            TokenKind::FatArrow,
            "expected `=>` after closure parameters",
        )?;
        let body = if self.at(TokenKind::LBrace) {
            Expr::Block(self.parse_block()?)
        } else {
            self.parse_expr()?
        };
        Ok(Expr::Closure {
            is_move,
            params,
            body: Box::new(body),
        })
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.at(TokenKind::LParen) {
                self.bump();
                let args = self.parse_call_args()?;
                self.expect(TokenKind::RParen, "expected `)` after call arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }

            if self.at(TokenKind::Dot) && self.nth_kind(1) == TokenKind::Ident {
                self.bump();
                let field = self.expect_ident("expected member name after `.`")?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    field,
                };
                continue;
            }

            if self.at(TokenKind::LBracket) {
                self.bump();
                let mut items = Vec::new();
                while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                    items.push(self.parse_expr()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "expected `]` after bracket expression")?;
                expr = Expr::Bracket {
                    target: Box::new(expr),
                    items,
                };
                continue;
            }

            if self.at(TokenKind::Question) {
                self.bump();
                expr = Expr::Question(Box::new(expr));
                continue;
            }

            if self.at(TokenKind::LBrace) && self.looks_like_struct_literal() {
                if let Some(path) = expr_to_path(&expr) {
                    self.bump();
                    let fields = self.parse_struct_literal_fields()?;
                    self.expect(TokenKind::RBrace, "expected `}` after struct literal")?;
                    expr = Expr::StructLiteral { path, fields };
                    continue;
                }
            }

            break;
        }

        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<CallArg>, ()> {
        let mut args = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Ident) && self.nth_kind(1) == TokenKind::Colon {
                let name = self.expect_ident("expected named argument label")?;
                self.expect(TokenKind::Colon, "expected `:` after named argument label")?;
                let value = self.parse_expr()?;
                args.push(CallArg::Named { name, value });
            } else {
                args.push(CallArg::Positional(self.parse_expr()?));
            }

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn parse_struct_literal_fields(&mut self) -> Result<Vec<StructLiteralField>, ()> {
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let name = self.expect_ident("expected field name in struct literal")?;
            let value = if self.eat(TokenKind::Colon) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            fields.push(StructLiteralField { name, value });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(fields)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ()> {
        match self.current().kind {
            TokenKind::Ident => {
                let name = self.bump().text;
                Ok(Expr::Name(name))
            }
            TokenKind::SelfKw => {
                let name = self.bump().text;
                Ok(Expr::Name(name))
            }
            TokenKind::Int => Ok(Expr::Integer(self.bump().text)),
            TokenKind::String => Ok(Expr::String {
                value: self.bump().text,
                is_format: false,
            }),
            TokenKind::FormatString => Ok(Expr::String {
                value: self.bump().text,
                is_format: true,
            }),
            TokenKind::TrueKw => {
                self.bump();
                Ok(Expr::Bool(true))
            }
            TokenKind::FalseKw => {
                self.bump();
                Ok(Expr::Bool(false))
            }
            TokenKind::NoneKw => {
                self.bump();
                Ok(Expr::NoneLiteral)
            }
            TokenKind::LBrace => Ok(Expr::Block(self.parse_block()?)),
            TokenKind::LBracket => {
                self.bump();
                let mut items = Vec::new();
                while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                    items.push(self.parse_expr()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "expected `]` after array literal")?;
                Ok(Expr::Array(items))
            }
            TokenKind::LParen => {
                self.bump();
                if self.at(TokenKind::RParen) {
                    self.bump();
                    return Ok(Expr::Tuple(Vec::new()));
                }

                let first = self.parse_expr()?;
                if self.eat(TokenKind::Comma) {
                    let mut items = vec![first];
                    while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                        items.push(self.parse_expr()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen, "expected `)` after tuple literal")?;
                    Ok(Expr::Tuple(items))
                } else {
                    self.expect(TokenKind::RParen, "expected `)` after expression")?;
                    Ok(first)
                }
            }
            _ => {
                self.error_here("expected expression");
                Err(())
            }
        }
    }

    fn is_closure_start(&self, start: usize) -> bool {
        let mut idx = start;
        if self.tokens.get(idx).map(|token| token.kind) == Some(TokenKind::MoveKw) {
            idx += 1;
        }

        if self.tokens.get(idx).map(|token| token.kind) != Some(TokenKind::LParen) {
            return false;
        }

        let mut depth = 0;
        while let Some(token) = self.tokens.get(idx) {
            match token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(idx + 1)
                            .map(|token| token.kind == TokenKind::FatArrow)
                            .unwrap_or(false);
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            idx += 1;
        }

        false
    }

    fn looks_like_struct_literal(&self) -> bool {
        if !self.at(TokenKind::LBrace) {
            return false;
        }

        matches!(
            (self.nth_kind(1), self.nth_kind(2)),
            (TokenKind::RBrace, _)
                | (TokenKind::Ident, TokenKind::Colon)
                | (TokenKind::Ident, TokenKind::Comma)
                | (TokenKind::Ident, TokenKind::RBrace)
        )
    }
}
