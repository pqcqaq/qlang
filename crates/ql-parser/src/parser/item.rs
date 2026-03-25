use ql_ast::{
    EnumDecl, EnumVariant, ExtendBlock, ExternBlock, FieldDecl, FunctionDecl, GenericParam,
    GlobalDecl, ImplBlock, Item, ItemKind, Param, ReceiverKind, StructDecl, TraitDecl,
    TypeAliasDecl, TypeExpr, TypeExprKind, VariantFields, Visibility, WherePredicate,
};
use ql_lexer::TokenKind;

use super::Parser;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionBodyMode {
    Required,
    Optional,
    Forbidden,
}

impl Parser {
    pub(super) fn parse_item(&mut self) -> Option<Item> {
        let start = self.current_start();
        let visibility = if self.eat(TokenKind::Pub) {
            Visibility::Public
        } else {
            Visibility::Private
        };

        let (is_async, is_unsafe) = self.parse_item_modifiers();

        let kind = match self.current().kind {
            TokenKind::Fn => {
                self.bump();
                self.parse_function_decl(
                    visibility,
                    is_async,
                    is_unsafe,
                    None,
                    FunctionBodyMode::Required,
                )
                .ok()
                .map(ItemKind::Function)
            }
            TokenKind::Const => {
                self.bump();
                self.parse_global_decl(visibility).ok().map(ItemKind::Const)
            }
            TokenKind::Static => {
                self.bump();
                self.parse_global_decl(visibility)
                    .ok()
                    .map(ItemKind::Static)
            }
            TokenKind::Type => {
                self.bump();
                self.parse_type_alias(visibility, false)
                    .ok()
                    .map(ItemKind::TypeAlias)
            }
            TokenKind::Opaque => {
                self.bump();
                self.expect(TokenKind::Type, "expected `type` after `opaque`")
                    .ok()?;
                self.parse_type_alias(visibility, true)
                    .ok()
                    .map(ItemKind::TypeAlias)
            }
            TokenKind::Data => {
                self.bump();
                self.expect(TokenKind::Struct, "expected `struct` after `data`")
                    .ok()?;
                self.parse_struct(visibility, true)
                    .ok()
                    .map(ItemKind::Struct)
            }
            TokenKind::Struct => {
                self.bump();
                self.parse_struct(visibility, false)
                    .ok()
                    .map(ItemKind::Struct)
            }
            TokenKind::Enum => {
                self.bump();
                self.parse_enum(visibility).ok().map(ItemKind::Enum)
            }
            TokenKind::Trait => {
                self.bump();
                self.parse_trait(visibility).ok().map(ItemKind::Trait)
            }
            TokenKind::Impl => {
                self.bump();
                self.parse_impl().ok().map(ItemKind::Impl)
            }
            TokenKind::Extend => {
                self.bump();
                self.parse_extend().ok().map(ItemKind::Extend)
            }
            TokenKind::Extern => {
                self.bump();
                self.parse_extern_item(visibility, is_async, is_unsafe).ok()
            }
            _ => {
                if is_async || is_unsafe {
                    self.error_here("expected `fn` after item modifier");
                } else {
                    self.error_here("expected item declaration");
                }
                None
            }
        }?;

        Some(Item::new(self.span_from(start), kind))
    }

    fn parse_item_modifiers(&mut self) -> (bool, bool) {
        let mut is_async = false;
        let mut is_unsafe = false;

        loop {
            if !is_async && self.eat(TokenKind::Async) {
                is_async = true;
                continue;
            }

            if !is_unsafe && self.eat(TokenKind::Unsafe) {
                is_unsafe = true;
                continue;
            }

            break;
        }

        (is_async, is_unsafe)
    }

    fn parse_global_decl(&mut self, visibility: Visibility) -> Result<GlobalDecl, ()> {
        let name = self.expect_ident_token("expected global name")?;
        self.expect(TokenKind::Colon, "expected `:` after global name")?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Eq, "expected `=` after global type")?;
        let value = self.parse_expr()?;
        self.eat(TokenKind::Semi);

        Ok(GlobalDecl {
            visibility,
            name: name.text,
            name_span: name.span,
            ty,
            value,
        })
    }

    fn parse_type_alias(
        &mut self,
        visibility: Visibility,
        is_opaque: bool,
    ) -> Result<TypeAliasDecl, ()> {
        let name = self.expect_ident_token("expected type alias name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::Eq, "expected `=` in type alias")?;
        let ty = self.parse_type()?;
        self.eat(TokenKind::Semi);

        Ok(TypeAliasDecl {
            visibility,
            is_opaque,
            name: name.text,
            name_span: name.span,
            generics,
            ty,
        })
    }

    fn parse_struct(&mut self, visibility: Visibility, is_data: bool) -> Result<StructDecl, ()> {
        let name = self.expect_ident_token("expected struct name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace, "expected `{` after struct name")?;
        let fields = self.parse_field_list(true)?;
        self.expect(TokenKind::RBrace, "expected `}` after struct fields")?;

        Ok(StructDecl {
            visibility,
            is_data,
            name: name.text,
            name_span: name.span,
            generics,
            fields,
        })
    }

    fn parse_enum(&mut self, visibility: Visibility) -> Result<EnumDecl, ()> {
        let name = self.expect_ident_token("expected enum name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace, "expected `{` after enum name")?;
        let mut variants = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let variant_name = self.expect_ident_token("expected enum variant name")?;
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
                name: variant_name.text,
                name_span: variant_name.span,
                fields,
            });

            self.eat(TokenKind::Comma);
        }

        self.expect(TokenKind::RBrace, "expected `}` after enum body")?;
        Ok(EnumDecl {
            visibility,
            name: name.text,
            name_span: name.span,
            generics,
            variants,
        })
    }

    fn parse_trait(&mut self, visibility: Visibility) -> Result<TraitDecl, ()> {
        let name = self.expect_ident_token("expected trait name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace, "expected `{` after trait name")?;
        let mut methods = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let method_visibility = if self.eat(TokenKind::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            let (is_async, is_unsafe) = self.parse_item_modifiers();
            self.expect(TokenKind::Fn, "expected `fn` in trait body")?;
            methods.push(self.parse_function_decl(
                method_visibility,
                is_async,
                is_unsafe,
                None,
                FunctionBodyMode::Optional,
            )?);
            self.eat(TokenKind::Semi);
        }

        self.expect(TokenKind::RBrace, "expected `}` after trait body")?;
        Ok(TraitDecl {
            visibility,
            name: name.text,
            name_span: name.span,
            generics,
            methods,
        })
    }

    fn parse_impl(&mut self) -> Result<ImplBlock, ()> {
        let generics = self.parse_generic_params()?;
        let first = self.parse_type()?;
        let (trait_ty, target) = if self.eat(TokenKind::For) {
            let target = self.parse_type()?;
            (Some(first), target)
        } else {
            (None, first)
        };
        let where_clause = self.parse_where_clause()?;

        self.expect(TokenKind::LBrace, "expected `{` after impl header")?;
        let mut methods = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let visibility = if self.eat(TokenKind::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            let (is_async, is_unsafe) = self.parse_item_modifiers();
            self.expect(TokenKind::Fn, "expected `fn` in impl block")?;
            methods.push(self.parse_function_decl(
                visibility,
                is_async,
                is_unsafe,
                None,
                FunctionBodyMode::Required,
            )?);
        }

        self.expect(TokenKind::RBrace, "expected `}` after impl block")?;
        Ok(ImplBlock {
            generics,
            trait_ty,
            target,
            where_clause,
            methods,
        })
    }

    fn parse_extend(&mut self) -> Result<ExtendBlock, ()> {
        let target = self.parse_type()?;
        self.expect(TokenKind::LBrace, "expected `{` after extend target")?;
        let mut methods = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let visibility = if self.eat(TokenKind::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            let (is_async, is_unsafe) = self.parse_item_modifiers();
            self.expect(TokenKind::Fn, "expected `fn` in extend block")?;
            methods.push(self.parse_function_decl(
                visibility,
                is_async,
                is_unsafe,
                None,
                FunctionBodyMode::Required,
            )?);
        }

        self.expect(TokenKind::RBrace, "expected `}` after extend block")?;
        Ok(ExtendBlock { target, methods })
    }

    fn parse_extern_item(
        &mut self,
        inherited_visibility: Visibility,
        is_async: bool,
        is_unsafe: bool,
    ) -> Result<ItemKind, ()> {
        if is_async {
            self.error_here("`extern` items cannot be `async`");
            return Err(());
        }

        let abi = self.expect_abi_string()?;
        if self.eat(TokenKind::LBrace) {
            let mut functions = Vec::new();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                let visibility = if self.eat(TokenKind::Pub) {
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                let (_, fn_is_unsafe) = self.parse_item_modifiers();
                self.expect(TokenKind::Fn, "expected `fn` in extern block")?;
                functions.push(self.parse_function_decl(
                    visibility,
                    false,
                    is_unsafe || fn_is_unsafe,
                    Some(abi.clone()),
                    FunctionBodyMode::Forbidden,
                )?);
                self.eat(TokenKind::Semi);
            }
            self.expect(TokenKind::RBrace, "expected `}` after extern block")?;
            return Ok(ItemKind::ExternBlock(ExternBlock {
                visibility: inherited_visibility,
                abi,
                functions,
            }));
        }

        let visibility = if self.eat(TokenKind::Pub) {
            Visibility::Public
        } else {
            inherited_visibility
        };
        let (_, fn_is_unsafe) = self.parse_item_modifiers();
        self.expect(TokenKind::Fn, "expected `fn` after extern ABI")?;
        let function = self.parse_function_decl(
            visibility,
            false,
            is_unsafe || fn_is_unsafe,
            Some(abi),
            FunctionBodyMode::Forbidden,
        )?;
        Ok(ItemKind::Function(function))
    }

    fn parse_function_decl(
        &mut self,
        visibility: Visibility,
        is_async: bool,
        is_unsafe: bool,
        abi: Option<String>,
        body_mode: FunctionBodyMode,
    ) -> Result<FunctionDecl, ()> {
        let name = self.expect_ident_token("expected function name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LParen, "expected `(` after function name")?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen, "expected `)` after parameter list")?;
        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let where_clause = self.parse_where_clause()?;
        let body = match body_mode {
            FunctionBodyMode::Required => Some(self.parse_block()?),
            FunctionBodyMode::Optional => {
                if self.at(TokenKind::LBrace) {
                    Some(self.parse_block()?)
                } else {
                    None
                }
            }
            FunctionBodyMode::Forbidden => None,
        };

        Ok(FunctionDecl {
            visibility,
            is_async,
            is_unsafe,
            abi,
            generics,
            name: name.text,
            name_span: name.span,
            params,
            return_type,
            where_clause,
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
                let name = self.expect_ident_token("expected parameter name")?;
                self.expect(TokenKind::Colon, "expected `:` after parameter name")?;
                let ty = self.parse_type()?;
                params.push(Param::Regular {
                    name: name.text,
                    name_span: name.span,
                    ty,
                });
            }

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_field_list(&mut self, allow_default: bool) -> Result<Vec<FieldDecl>, ()> {
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let name = self.expect_ident_token("expected field name")?;
            self.expect(TokenKind::Colon, "expected `:` after field name")?;
            let ty = self.parse_type()?;
            let default = if allow_default && self.eat(TokenKind::Eq) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            fields.push(FieldDecl {
                name: name.text,
                name_span: name.span,
                ty,
                default,
            });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(fields)
    }

    fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ()> {
        if !self.eat(TokenKind::LBracket) {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            let name = self.expect_ident_token("expected generic parameter name")?;
            let bounds = if self.eat(TokenKind::Colon) {
                self.parse_bound_list()?
            } else {
                Vec::new()
            };
            params.push(GenericParam {
                name: name.text,
                name_span: name.span,
                bounds,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        self.expect(TokenKind::RBracket, "expected `]` after generic parameters")?;
        Ok(params)
    }

    fn parse_where_clause(&mut self) -> Result<Vec<WherePredicate>, ()> {
        if !self.eat(TokenKind::Where) {
            return Ok(Vec::new());
        }

        let mut predicates = Vec::new();
        while !self.at(TokenKind::LBrace)
            && !self.at(TokenKind::Semi)
            && !self.at(TokenKind::Eof)
            && !self.at(TokenKind::RParen)
        {
            let target = self.parse_type()?;
            self.expect(TokenKind::Colon, "expected `:` in where predicate")?;
            let bounds = self.parse_bound_list()?;
            predicates.push(WherePredicate { target, bounds });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        Ok(predicates)
    }

    fn parse_bound_list(&mut self) -> Result<Vec<ql_ast::Path>, ()> {
        let mut bounds = vec![self.parse_path()?];
        while self.eat(TokenKind::Plus) {
            bounds.push(self.parse_path()?);
        }
        Ok(bounds)
    }

    fn expect_abi_string(&mut self) -> Result<String, ()> {
        if self.at(TokenKind::String) {
            Ok(self.bump().text)
        } else {
            self.error_here("expected ABI string after `extern`");
            Err(())
        }
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ()> {
        let start = self.current_start();
        if self.eat(TokenKind::Star) {
            let is_const = self.eat(TokenKind::Const);
            let inner = self.parse_type()?;
            return Ok(TypeExpr::new(
                self.span_from(start),
                TypeExprKind::Pointer {
                    is_const,
                    inner: Box::new(inner),
                },
            ));
        }

        if self.eat(TokenKind::LParen) {
            let mut inner = Vec::new();
            let mut saw_comma = false;
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                inner.push(self.parse_type()?);
                if self.eat(TokenKind::Comma) {
                    saw_comma = true;
                } else {
                    break;
                }
            }
            self.expect(TokenKind::RParen, "expected `)` after type list")?;

            if self.eat(TokenKind::Arrow) {
                let ret = self.parse_type()?;
                return Ok(TypeExpr::new(
                    self.span_from(start),
                    TypeExprKind::Callable {
                        params: inner,
                        ret: Box::new(ret),
                    },
                ));
            }

            if inner.len() == 1 && !saw_comma {
                return Ok(inner.into_iter().next().unwrap());
            }

            return Ok(TypeExpr::new(
                self.span_from(start),
                TypeExprKind::Tuple(inner),
            ));
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

        Ok(TypeExpr::new(
            self.span_from(start),
            TypeExprKind::Named { path, args },
        ))
    }
}
