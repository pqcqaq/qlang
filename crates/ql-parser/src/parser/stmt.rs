use ql_ast::{Block, Stmt};
use ql_lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn parse_block(&mut self) -> Result<Block, ()> {
        self.expect(TokenKind::LBrace, "expected `{` to start block")?;
        let mut statements = Vec::new();
        let mut tail = None;

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Let) || self.at(TokenKind::Var) {
                let mutable = self.eat(TokenKind::Var);
                if !mutable {
                    self.expect(TokenKind::Let, "expected `let` or `var`")?;
                }
                let pattern = self.parse_pattern()?;
                self.expect(TokenKind::Eq, "expected `=` after pattern")?;
                let value = self.parse_expr()?;
                self.eat(TokenKind::Semi);
                statements.push(Stmt::Let {
                    mutable,
                    pattern,
                    value,
                });
                continue;
            }

            if self.eat(TokenKind::Return) {
                let value = if self.can_start_expr() {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.eat(TokenKind::Semi);
                statements.push(Stmt::Return(value));
                continue;
            }

            if self.eat(TokenKind::Defer) {
                let expr = self.parse_expr()?;
                self.eat(TokenKind::Semi);
                statements.push(Stmt::Defer(expr));
                continue;
            }

            if self.eat(TokenKind::Break) {
                self.eat(TokenKind::Semi);
                statements.push(Stmt::Break);
                continue;
            }

            if self.eat(TokenKind::Continue) {
                self.eat(TokenKind::Semi);
                statements.push(Stmt::Continue);
                continue;
            }

            if self.eat(TokenKind::While) {
                let condition = self.parse_expr()?;
                let body = self.parse_block()?;
                statements.push(Stmt::While { condition, body });
                continue;
            }

            if self.eat(TokenKind::Loop) {
                let body = self.parse_block()?;
                statements.push(Stmt::Loop { body });
                continue;
            }

            if self.eat(TokenKind::For) {
                let is_await = self.eat(TokenKind::Await);
                let pattern = self.parse_pattern()?;
                self.expect(TokenKind::In, "expected `in` in `for` loop")?;
                let iterable = self.parse_expr()?;
                let body = self.parse_block()?;
                statements.push(Stmt::For {
                    is_await,
                    pattern,
                    iterable,
                    body,
                });
                continue;
            }

            let expr = self.parse_expr()?;
            if self.eat(TokenKind::Semi) {
                statements.push(Stmt::Expr {
                    expr,
                    terminated: true,
                });
            } else if self.at(TokenKind::RBrace) {
                tail = Some(Box::new(expr));
                break;
            } else if self.starts_statement() {
                statements.push(Stmt::Expr {
                    expr,
                    terminated: false,
                });
            } else {
                self.error_here("expected `;` or end of block");
                statements.push(Stmt::Expr {
                    expr,
                    terminated: false,
                });
            }
        }

        self.expect(TokenKind::RBrace, "expected `}` after block")?;
        Ok(Block { statements, tail })
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
                | TokenKind::Minus
        )
    }
}
