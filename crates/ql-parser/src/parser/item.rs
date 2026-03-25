use ql_ast::{
    EnumDecl, EnumVariant, FieldDecl, FunctionDecl, ImplBlock, Item, Param, ReceiverKind,
    StructDecl, TypeExpr, VariantFields, Visibility,
};
use ql_lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn parse_item(&mut self) -> Option<Item> {
        let visibility = if self.eat(TokenKind::Pub) {
            Visibility::Public
        } else {
            Visibility::Private
        };

        if self.eat(TokenKind::Async) {
            self.expect(TokenKind::Fn, "expected `fn` after `async`")
                .ok()?;
            return self
                .parse_function(visibility, true)
                .ok()
                .map(Item::Function);
        }

        match self.current().kind {
            TokenKind::Fn => {
                self.bump();
                self.parse_function(visibility, false)
                    .ok()
                    .map(Item::Function)
            }
            TokenKind::Data => {
                self.bump();
                self.expect(TokenKind::Struct, "expected `struct` after `data`")
                    .ok()?;
                self.parse_struct(visibility, true).ok().map(Item::Struct)
            }
            TokenKind::Struct => {
                self.bump();
                self.parse_struct(visibility, false).ok().map(Item::Struct)
            }
            TokenKind::Enum => {
                self.bump();
                self.parse_enum(visibility).ok().map(Item::Enum)
            }
            TokenKind::Impl => {
                self.bump();
                self.parse_impl().ok().map(Item::Impl)
            }
            _ => {
                self.error_here("expected item declaration");
                None
            }
        }
    }

    fn parse_function(
        &mut self,
        visibility: Visibility,
        is_async: bool,
    ) -> Result<FunctionDecl, ()> {
        let name = self.expect_ident("expected function name")?;
        self.expect(TokenKind::LParen, "expected `(` after function name")?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen, "expected `)` after parameter list")?;
        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;

        Ok(FunctionDecl {
            visibility,
            is_async,
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ()> {
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::SelfKw)
                || (self.at(TokenKind::Var) && self.nth_kind(1) == TokenKind::SelfKw)
                || (self.at(TokenKind::MoveKw) && self.nth_kind(1) == TokenKind::SelfKw)
            {
                let receiver = if self.eat(TokenKind::Var) {
                    self.expect(TokenKind::SelfKw, "expected `self` after `var`")?;
                    ReceiverKind::Mutable
                } else if self.eat(TokenKind::MoveKw) {
                    self.expect(TokenKind::SelfKw, "expected `self` after `move`")?;
                    ReceiverKind::Move
                } else {
                    self.expect(TokenKind::SelfKw, "expected `self`")?;
                    ReceiverKind::ReadOnly
                };
                params.push(Param::Receiver(receiver));
            } else {
                let name = self.expect_ident("expected parameter name")?;
                self.expect(TokenKind::Colon, "expected `:` after parameter name")?;
                let ty = self.parse_type()?;
                params.push(Param::Regular { name, ty });
            }

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_struct(&mut self, visibility: Visibility, is_data: bool) -> Result<StructDecl, ()> {
        let name = self.expect_ident("expected struct name")?;
        self.expect(TokenKind::LBrace, "expected `{` after struct name")?;
        let fields = self.parse_field_list(true)?;
        self.expect(TokenKind::RBrace, "expected `}` after struct fields")?;

        Ok(StructDecl {
            visibility,
            is_data,
            name,
            fields,
        })
    }

    fn parse_enum(&mut self, visibility: Visibility) -> Result<EnumDecl, ()> {
        let name = self.expect_ident("expected enum name")?;
        self.expect(TokenKind::LBrace, "expected `{` after enum name")?;
        let mut variants = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let variant_name = self.expect_ident("expected enum variant name")?;
            let fields = if self.eat(TokenKind::LParen) {
                let mut types = Vec::new();
                while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                    types.push(self.parse_type()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "expected `)` after enum tuple fields")?;
                VariantFields::Tuple(types)
            } else if self.eat(TokenKind::LBrace) {
                let fields = self.parse_field_list(false)?;
                self.expect(TokenKind::RBrace, "expected `}` after enum named fields")?;
                VariantFields::Struct(fields)
            } else {
                VariantFields::Unit
            };

            variants.push(EnumVariant {
                name: variant_name,
                fields,
            });

            self.eat(TokenKind::Comma);
        }

        self.expect(TokenKind::RBrace, "expected `}` after enum body")?;
        Ok(EnumDecl {
            visibility,
            name,
            variants,
        })
    }

    fn parse_impl(&mut self) -> Result<ImplBlock, ()> {
        let target = self.parse_path()?;
        self.expect(TokenKind::LBrace, "expected `{` after impl target")?;
        let mut methods = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let visibility = if self.eat(TokenKind::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            let is_async = self.eat(TokenKind::Async);
            self.expect(TokenKind::Fn, "expected `fn` in impl block")?;
            methods.push(self.parse_function(visibility, is_async)?);
        }

        self.expect(TokenKind::RBrace, "expected `}` after impl block")?;
        Ok(ImplBlock { target, methods })
    }

    fn parse_field_list(&mut self, allow_default: bool) -> Result<Vec<FieldDecl>, ()> {
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let name = self.expect_ident("expected field name")?;
            self.expect(TokenKind::Colon, "expected `:` after field name")?;
            let ty = self.parse_type()?;
            let default = if allow_default && self.eat(TokenKind::Eq) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            fields.push(FieldDecl { name, ty, default });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(fields)
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ()> {
        if self.eat(TokenKind::LParen) {
            let mut inner = Vec::new();
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                inner.push(self.parse_type()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "expected `)` after type list")?;

            if self.eat(TokenKind::Arrow) {
                let ret = self.parse_type()?;
                return Ok(TypeExpr::Callable {
                    params: inner,
                    ret: Box::new(ret),
                });
            }

            if inner.len() == 1 {
                return Ok(inner.into_iter().next().unwrap());
            }
            return Ok(TypeExpr::Tuple(inner));
        }

        let path = self.parse_path()?;
        let args = if self.eat(TokenKind::LBracket) {
            let mut args = Vec::new();
            while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                args.push(self.parse_type()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBracket, "expected `]` after generic arguments")?;
            args
        } else {
            Vec::new()
        };

        Ok(TypeExpr::Named { path, args })
    }
}
