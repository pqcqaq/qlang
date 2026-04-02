use ql_ast::{Block, Stmt, StmtKind};
use ql_lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn parse_block(&mut self) -> Result<Block, ()> {
        let start = self.current_start();
        self.expect(TokenKind::LBrace, "expected `{` to start block")?;
        let mut statements = Vec::new();
        let mut tail = None;

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Let) || self.at(TokenKind::Var) {
                let stmt_start = self.current_start();
                let mutable = self.eat(TokenKind::Var);
                if !mutable {
                    self.expect(TokenKind::Let, "expected `let` or `var`")?;
                }
                let pattern = self.parse_pattern()?;
                self.expect(TokenKind::Eq, "expected `=` after pattern")?;
                let value = self.parse_expr()?;
                self.eat(TokenKind::Semi);
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::Let {
                        mutable,
                        pattern,
                        value,
                    },
                ));
                continue;
            }

            if self.eat(TokenKind::Return) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                let value = if self.can_start_expr() {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.eat(TokenKind::Semi);
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::Return(value),
                ));
                continue;
            }

            if self.eat(TokenKind::Defer) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                let expr = self.parse_expr()?;
                self.eat(TokenKind::Semi);
                statements.push(Stmt::new(self.span_from(stmt_start), StmtKind::Defer(expr)));
                continue;
            }

            if self.eat(TokenKind::Break) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                self.eat(TokenKind::Semi);
                statements.push(Stmt::new(self.span_from(stmt_start), StmtKind::Break));
                continue;
            }

            if self.eat(TokenKind::Continue) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                self.eat(TokenKind::Semi);
                statements.push(Stmt::new(self.span_from(stmt_start), StmtKind::Continue));
                continue;
            }

            if self.eat(TokenKind::While) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                let condition = self.parse_head_expr()?;
                let body = self.parse_block()?;
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::While { condition, body },
                ));
                continue;
            }

            if self.eat(TokenKind::Loop) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                let body = self.parse_block()?;
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::Loop { body },
                ));
                continue;
            }

            if self.eat(TokenKind::For) {
                let stmt_start = self.tokens[self.idx.saturating_sub(1)].span.start;
                let is_await = self.eat(TokenKind::Await);
                let pattern = self.parse_pattern()?;
                self.expect(TokenKind::In, "expected `in` in `for` loop")?;
                let iterable = self.parse_head_expr()?;
                let body = self.parse_block()?;
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::For {
                        is_await,
                        pattern,
                        iterable,
                        body,
                    },
                ));
                continue;
            }

            let stmt_start = self.current_start();
            let expr = self.parse_expr()?;
            if self.eat(TokenKind::Semi) {
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::Expr {
                        expr,
                        terminated: true,
                    },
                ));
            } else if self.at(TokenKind::RBrace) {
                tail = Some(Box::new(expr));
                break;
            } else if self.starts_statement() {
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::Expr {
                        expr,
                        terminated: false,
                    },
                ));
            } else {
                self.error_here("expected `;` or end of block");
                statements.push(Stmt::new(
                    self.span_from(stmt_start),
                    StmtKind::Expr {
                        expr,
                        terminated: false,
                    },
                ));
            }
        }

        self.expect(TokenKind::RBrace, "expected `}` after block")?;
        Ok(Block {
            span: self.span_from(start),
            statements,
            tail,
        })
    }

    fn starts_statement(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Let
                | TokenKind::Var
                | TokenKind::Return
                | TokenKind::Defer
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::While
                | TokenKind::Loop
                | TokenKind::For
                | TokenKind::If
                | TokenKind::Match
        )
    }

    fn can_start_expr(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Ident
                | TokenKind::SelfKw
                | TokenKind::Int
                | TokenKind::String
                | TokenKind::FormatString
                | TokenKind::TrueKw
                | TokenKind::FalseKw
                | TokenKind::NoneKw
                | TokenKind::If
                | TokenKind::Match
                | TokenKind::LBrace
                | TokenKind::LBracket
                | TokenKind::LParen
                | TokenKind::Unsafe
                | TokenKind::MoveKw
                | TokenKind::Await
                | TokenKind::Spawn
                | TokenKind::Bang
                | TokenKind::Minus
        )
    }
}
