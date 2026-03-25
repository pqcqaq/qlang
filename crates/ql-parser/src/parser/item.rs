use ql_ast::{
    EnumDecl, EnumVariant, ExtendBlock, ExternBlock, FieldDecl, FunctionDecl, GenericParam,
    GlobalDecl, ImplBlock, Item, Param, ReceiverKind, StructDecl, TraitDecl, TypeAliasDecl,
    TypeExpr, VariantFields, Visibility, WherePredicate,
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
        let visibility = if self.eat(TokenKind::Pub) {
            Visibility::Public
        } else {
            Visibility::Private
        };

        let (is_async, is_unsafe) = self.parse_item_modifiers();

        match self.current().kind {
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
                .map(Item::Function)
            }
            TokenKind::Const => {
                self.bump();
                self.parse_global_decl(visibility).ok().map(Item::Const)
            }
            TokenKind::Static => {
                self.bump();
                self.parse_global_decl(visibility).ok().map(Item::Static)
            }
            TokenKind::Type => {
                self.bump();
                self.parse_type_alias(visibility, false)
                    .ok()
                    .map(Item::TypeAlias)
            }
            TokenKind::Opaque => {
                self.bump();
                self.expect(TokenKind::Type, "expected `type` after `opaque`")
                    .ok()?;
                self.parse_type_alias(visibility, true)
                    .ok()
                    .map(Item::TypeAlias)
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
            TokenKind::Trait => {
                self.bump();
                self.parse_trait(visibility).ok().map(Item::Trait)
            }
            TokenKind::Impl => {
                self.bump();
                self.parse_impl().ok().map(Item::Impl)
            }
            TokenKind::Extend => {
                self.bump();
                self.parse_extend().ok().map(Item::Extend)
            }
            TokenKind::Extern => {
                self.bump();
                self.parse_extern_item(visibility, is_async, is_unsafe)
                    .ok()
                    .map(|item| item)
            }
            _ => {
                if is_async || is_unsafe {
                    self.error_here("expected `fn` after item modifier");
                } else {
                    self.error_here("expected item declaration");
                }
                None
            }
        }
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
        let name = self.expect_ident("expected global name")?;
        self.expect(TokenKind::Colon, "expected `:` after global name")?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Eq, "expected `=` after global type")?;
        let value = self.parse_expr()?;
        self.eat(TokenKind::Semi);

        Ok(GlobalDecl {
            visibility,
            name,
            ty,
            value,
        })
    }

    fn parse_type_alias(
        &mut self,
        visibility: Visibility,
        is_opaque: bool,
    ) -> Result<TypeAliasDecl, ()> {
        let name = self.expect_ident("expected type alias name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::Eq, "expected `=` in type alias")?;
        let ty = self.parse_type()?;
        self.eat(TokenKind::Semi);

        Ok(TypeAliasDecl {
            visibility,
            is_opaque,
            name,
            generics,
            ty,
        })
    }

    fn parse_struct(&mut self, visibility: Visibility, is_data: bool) -> Result<StructDecl, ()> {
        let name = self.expect_ident("expected struct name")?;
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LBrace, "expected `{` after struct name")?;
        let fields = self.parse_field_list(true)?;
        self.expect(TokenKind::RBrace, "expected `}` after struct fields")?;

        Ok(StructDecl {
            visibility,
            is_data,
            name,
            generics,
            fields,
        })
    }

    fn parse_enum(&mut self, visibility: Visibility) -> Result<EnumDecl, ()> {
        let name = self.expect_ident("expected enum name")?;
        let generics = self.parse_generic_params()?;
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
            generics,
            variants,
        })
    }

    fn parse_trait(&mut self, visibility: Visibility) -> Result<TraitDecl, ()> {
        let name = self.expect_ident("expected trait name")?;
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
            name,
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
    ) -> Result<Item, ()> {
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
            return Ok(Item::ExternBlock(ExternBlock { abi, functions }));
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
        Ok(Item::Function(function))
    }

    fn parse_function_decl(
        &mut self,
        visibility: Visibility,
        is_async: bool,
        is_unsafe: bool,
        abi: Option<String>,
        body_mode: FunctionBodyMode,
    ) -> Result<FunctionDecl, ()> {
        let name = self.expect_ident("expected function name")?;
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
            name,
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

    fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ()> {
        if !self.eat(TokenKind::LBracket) {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            let name = self.expect_ident("expected generic parameter name")?;
            let bounds = if self.eat(TokenKind::Colon) {
                self.parse_bound_list()?
            } else {
                Vec::new()
            };
            params.push(GenericParam { name, bounds });
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
