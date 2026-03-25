use ql_ast::{BinaryOp, CallArg, Expr, ExprKind, MatchArm, StructLiteralField};
use ql_lexer::TokenKind;
use ql_span::Span;

use super::{Parser, expr_to_path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StructLiteralMode {
    Allow,
    Disallow,
}

impl Parser {
    pub(super) fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_expr_with_struct_literals(StructLiteralMode::Allow)
    }

    pub(super) fn parse_head_expr(&mut self) -> Result<Expr, ()> {
        self.parse_expr_with_struct_literals(StructLiteralMode::Disallow)
    }

    fn parse_expr_with_struct_literals(
        &mut self,
        struct_literal_mode: StructLiteralMode,
    ) -> Result<Expr, ()> {
        if self.at(TokenKind::If) {
            return self.parse_if_expr();
        }

        if self.at(TokenKind::Match) {
            return self.parse_match_expr();
        }

        self.parse_binary_expr(0, struct_literal_mode)
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ()> {
        let start = self.current_start();
        self.expect(TokenKind::If, "expected `if`")?;
        let condition = self.parse_head_expr()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.eat(TokenKind::Else) {
            if self.at(TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else if self.at(TokenKind::LBrace) {
                let block = self.parse_block()?;
                Some(Box::new(Expr::new(block.span, ExprKind::Block(block))))
            } else {
                self.error_here("expected `if` or block after `else`");
                return Err(());
            }
        } else {
            None
        };

        Ok(Expr::new(
            self.span_from(start),
            ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch,
            },
        ))
    }

    fn parse_match_expr(&mut self) -> Result<Expr, ()> {
        let start = self.current_start();
        self.expect(TokenKind::Match, "expected `match`")?;
        let value = self.parse_head_expr()?;
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
                let block = self.parse_block()?;
                Expr::new(block.span, ExprKind::Block(block))
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
        Ok(Expr::new(
            self.span_from(start),
            ExprKind::Match {
                value: Box::new(value),
                arms,
            },
        ))
    }

    fn parse_binary_expr(
        &mut self,
        min_prec: u8,
        struct_literal_mode: StructLiteralMode,
    ) -> Result<Expr, ()> {
        let mut left = self.parse_prefix_expr(struct_literal_mode)?;

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
            let start = left.span.start;
            let right = self.parse_binary_expr(next_min_prec, struct_literal_mode)?;
            left = Expr::new(
                Span::new(start, right.span.end),
                ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            );
        }

        Ok(left)
    }

    fn parse_prefix_expr(&mut self, struct_literal_mode: StructLiteralMode) -> Result<Expr, ()> {
        if self.at(TokenKind::MoveKw) && self.is_closure_start(self.idx) {
            return self.parse_closure();
        }

        match self.current().kind {
            TokenKind::Unsafe => {
                let start = self.current_start();
                self.bump();
                Ok(Expr::new(
                    self.span_from(start),
                    ExprKind::Unsafe(self.parse_block()?),
                ))
            }
            TokenKind::Await => {
                let start = self.current_start();
                self.bump();
                let expr = self.parse_prefix_expr(struct_literal_mode)?;
                Ok(Expr::new(
                    self.span_from(start),
                    ExprKind::Unary {
                        op: ql_ast::UnaryOp::Await,
                        expr: Box::new(expr),
                    },
                ))
            }
            TokenKind::Spawn => {
                let start = self.current_start();
                self.bump();
                let expr = self.parse_prefix_expr(struct_literal_mode)?;
                Ok(Expr::new(
                    self.span_from(start),
                    ExprKind::Unary {
                        op: ql_ast::UnaryOp::Spawn,
                        expr: Box::new(expr),
                    },
                ))
            }
            TokenKind::Minus => {
                let start = self.current_start();
                self.bump();
                let expr = self.parse_prefix_expr(struct_literal_mode)?;
                Ok(Expr::new(
                    self.span_from(start),
                    ExprKind::Unary {
                        op: ql_ast::UnaryOp::Neg,
                        expr: Box::new(expr),
                    },
                ))
            }
            TokenKind::LParen if self.is_closure_start(self.idx) => self.parse_closure(),
            _ => self.parse_postfix_expr(struct_literal_mode),
        }
    }

    fn parse_closure(&mut self) -> Result<Expr, ()> {
        let start = self.current_start();
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
            let block = self.parse_block()?;
            Expr::new(block.span, ExprKind::Block(block))
        } else {
            self.parse_expr()?
        };
        Ok(Expr::new(
            self.span_from(start),
            ExprKind::Closure {
                is_move,
                params,
                body: Box::new(body),
            },
        ))
    }

    fn parse_postfix_expr(&mut self, struct_literal_mode: StructLiteralMode) -> Result<Expr, ()> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.at(TokenKind::LParen) {
                let start = expr.span.start;
                self.bump();
                let args = self.parse_call_args()?;
                self.expect(TokenKind::RParen, "expected `)` after call arguments")?;
                expr = Expr::new(
                    Span::new(start, self.previous_end()),
                    ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                );
                continue;
            }

            if self.at(TokenKind::Dot) && self.nth_kind(1) == TokenKind::Ident {
                let start = expr.span.start;
                self.bump();
                let field = self.expect_ident("expected member name after `.`")?;
                expr = Expr::new(
                    Span::new(start, self.previous_end()),
                    ExprKind::Member {
                        object: Box::new(expr),
                        field,
                    },
                );
                continue;
            }

            if self.at(TokenKind::LBracket) {
                let start = expr.span.start;
                self.bump();
                let mut items = Vec::new();
                while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                    items.push(self.parse_expr()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "expected `]` after bracket expression")?;
                expr = Expr::new(
                    Span::new(start, self.previous_end()),
                    ExprKind::Bracket {
                        target: Box::new(expr),
                        items,
                    },
                );
                continue;
            }

            if self.at(TokenKind::Question) {
                let start = expr.span.start;
                self.bump();
                expr = Expr::new(
                    Span::new(start, self.previous_end()),
                    ExprKind::Question(Box::new(expr)),
                );
                continue;
            }

            if struct_literal_mode == StructLiteralMode::Allow
                && self.at(TokenKind::LBrace)
                && self.looks_like_struct_literal()
            {
                if let Some(path) = expr_to_path(&expr) {
                    let start = expr.span.start;
                    self.bump();
                    let fields = self.parse_struct_literal_fields()?;
                    self.expect(TokenKind::RBrace, "expected `}` after struct literal")?;
                    expr = Expr::new(
                        Span::new(start, self.previous_end()),
                        ExprKind::StructLiteral { path, fields },
                    );
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
        let start = self.current_start();
        match self.current().kind {
            TokenKind::Ident => {
                let token = self.bump();
                Ok(Expr::new(token.span, ExprKind::Name(token.text)))
            }
            TokenKind::SelfKw => {
                let token = self.bump();
                Ok(Expr::new(token.span, ExprKind::Name(token.text)))
            }
            TokenKind::Int => {
                let token = self.bump();
                Ok(Expr::new(token.span, ExprKind::Integer(token.text)))
            }
            TokenKind::String => {
                let token = self.bump();
                Ok(Expr::new(
                    token.span,
                    ExprKind::String {
                        value: token.text,
                        is_format: false,
                    },
                ))
            }
            TokenKind::FormatString => {
                let token = self.bump();
                Ok(Expr::new(
                    token.span,
                    ExprKind::String {
                        value: token.text,
                        is_format: true,
                    },
                ))
            }
            TokenKind::TrueKw => {
                let token = self.bump();
                Ok(Expr::new(token.span, ExprKind::Bool(true)))
            }
            TokenKind::FalseKw => {
                let token = self.bump();
                Ok(Expr::new(token.span, ExprKind::Bool(false)))
            }
            TokenKind::NoneKw => {
                let token = self.bump();
                Ok(Expr::new(token.span, ExprKind::NoneLiteral))
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Expr::new(block.span, ExprKind::Block(block)))
            }
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
                Ok(Expr::new(self.span_from(start), ExprKind::Array(items)))
            }
            TokenKind::LParen => {
                self.bump();
                if self.at(TokenKind::RParen) {
                    self.bump();
                    return Ok(Expr::new(
                        self.span_from(start),
                        ExprKind::Tuple(Vec::new()),
                    ));
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
                    Ok(Expr::new(self.span_from(start), ExprKind::Tuple(items)))
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
