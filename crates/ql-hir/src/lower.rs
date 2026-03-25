use ql_ast as ast;
use ql_span::Span;

use crate::{
    Block, CallArg, Enum, EnumVariant, Expr, ExprKind, Extend, ExternBlock, Field, Function,
    GenericParam, Global, Impl, Item, ItemId, ItemKind, Local, MatchArm, Module, Param, Pattern,
    PatternField, PatternKind, ReceiverParam, RegularParam, Stmt, StmtKind, Struct,
    StructLiteralField, Trait, Type, TypeAlias, TypeId, TypeKind, VariantFields, WherePredicate,
};

/// Lower a parsed AST module into the semantic HIR container used by later phases.
pub fn lower_module(ast: &ast::Module) -> Module {
    Lowerer::new().lower_module(ast)
}

struct Lowerer {
    module: Module,
}

impl Lowerer {
    fn new() -> Self {
        Self {
            module: Module::default(),
        }
    }

    fn lower_module(mut self, ast: &ast::Module) -> Module {
        self.module.package = ast.package.clone();
        self.module.uses = ast.uses.clone();

        for item in &ast.items {
            let item_id = self.lower_item(item);
            self.module.items.push(item_id);
        }

        self.module
    }

    fn lower_item(&mut self, item: &ast::Item) -> ItemId {
        let kind = match &item.kind {
            ast::ItemKind::Function(function) => {
                ItemKind::Function(self.lower_function(function, item.span))
            }
            ast::ItemKind::Const(global) => ItemKind::Const(self.lower_global(global, item.span)),
            ast::ItemKind::Static(global) => ItemKind::Static(self.lower_global(global, item.span)),
            ast::ItemKind::Struct(struct_decl) => {
                ItemKind::Struct(self.lower_struct(struct_decl, item.span))
            }
            ast::ItemKind::Enum(enum_decl) => ItemKind::Enum(self.lower_enum(enum_decl, item.span)),
            ast::ItemKind::Trait(trait_decl) => {
                ItemKind::Trait(self.lower_trait(trait_decl, item.span))
            }
            ast::ItemKind::Impl(impl_block) => {
                ItemKind::Impl(self.lower_impl(impl_block, item.span))
            }
            ast::ItemKind::Extend(extend_block) => {
                ItemKind::Extend(self.lower_extend(extend_block, item.span))
            }
            ast::ItemKind::TypeAlias(alias) => {
                ItemKind::TypeAlias(self.lower_type_alias(alias, item.span))
            }
            ast::ItemKind::ExternBlock(extern_block) => {
                ItemKind::ExternBlock(self.lower_extern_block(extern_block, item.span))
            }
        };

        self.module.alloc_item(Item {
            span: item.span,
            kind,
        })
    }

    fn lower_function(&mut self, function: &ast::FunctionDecl, span: Span) -> Function {
        Function {
            span,
            visibility: function.visibility.clone(),
            is_async: function.is_async,
            is_unsafe: function.is_unsafe,
            abi: function.abi.clone(),
            generics: self.lower_generics(&function.generics, span),
            name: function.name.clone(),
            params: function
                .params
                .iter()
                .map(|param| self.lower_param(param, span))
                .collect(),
            return_type: function
                .return_type
                .as_ref()
                .map(|ty| self.lower_type_expr(ty)),
            where_clause: self.lower_where_clause(&function.where_clause),
            body: function.body.as_ref().map(|block| self.lower_block(block)),
        }
    }

    fn lower_param(&mut self, param: &ast::Param, fallback_span: Span) -> Param {
        match param {
            ast::Param::Regular { name, ty } => Param::Regular(RegularParam {
                name: name.clone(),
                span: ty.span,
                ty: self.lower_type_expr(ty),
            }),
            ast::Param::Receiver(kind) => Param::Receiver(ReceiverParam {
                kind: *kind,
                span: fallback_span,
            }),
        }
    }

    fn lower_generics(
        &mut self,
        generics: &[ast::GenericParam],
        fallback_span: Span,
    ) -> Vec<GenericParam> {
        generics
            .iter()
            .map(|generic| GenericParam {
                name: generic.name.clone(),
                span: fallback_span,
                bounds: generic.bounds.clone(),
            })
            .collect()
    }

    fn lower_where_clause(&mut self, predicates: &[ast::WherePredicate]) -> Vec<WherePredicate> {
        predicates
            .iter()
            .map(|predicate| WherePredicate {
                target: self.lower_type_expr(&predicate.target),
                bounds: predicate.bounds.clone(),
            })
            .collect()
    }

    fn lower_global(&mut self, global: &ast::GlobalDecl, span: Span) -> Global {
        Global {
            span,
            visibility: global.visibility.clone(),
            name: global.name.clone(),
            ty: self.lower_type_expr(&global.ty),
            value: self.lower_expr(&global.value),
        }
    }

    fn lower_struct(&mut self, struct_decl: &ast::StructDecl, span: Span) -> Struct {
        Struct {
            span,
            visibility: struct_decl.visibility.clone(),
            is_data: struct_decl.is_data,
            name: struct_decl.name.clone(),
            generics: self.lower_generics(&struct_decl.generics, span),
            fields: struct_decl
                .fields
                .iter()
                .map(|field| self.lower_field(field, span))
                .collect(),
        }
    }

    fn lower_field(&mut self, field: &ast::FieldDecl, fallback_span: Span) -> Field {
        let default = field.default.as_ref().map(|expr| self.lower_expr(expr));
        let span = field
            .default
            .as_ref()
            .map(|expr| expr.span)
            .unwrap_or(field.ty.span);

        Field {
            name: field.name.clone(),
            span: prefer_span(span, fallback_span),
            ty: self.lower_type_expr(&field.ty),
            default,
        }
    }

    fn lower_enum(&mut self, enum_decl: &ast::EnumDecl, span: Span) -> Enum {
        Enum {
            span,
            visibility: enum_decl.visibility.clone(),
            name: enum_decl.name.clone(),
            generics: self.lower_generics(&enum_decl.generics, span),
            variants: enum_decl
                .variants
                .iter()
                .map(|variant| self.lower_variant(variant, span))
                .collect(),
        }
    }

    fn lower_variant(&mut self, variant: &ast::EnumVariant, fallback_span: Span) -> EnumVariant {
        let fields = match &variant.fields {
            ast::VariantFields::Unit => VariantFields::Unit,
            ast::VariantFields::Tuple(items) => {
                VariantFields::Tuple(items.iter().map(|ty| self.lower_type_expr(ty)).collect())
            }
            ast::VariantFields::Struct(fields) => VariantFields::Struct(
                fields
                    .iter()
                    .map(|field| self.lower_field(field, fallback_span))
                    .collect(),
            ),
        };

        EnumVariant {
            name: variant.name.clone(),
            span: fallback_span,
            fields,
        }
    }

    fn lower_trait(&mut self, trait_decl: &ast::TraitDecl, span: Span) -> Trait {
        Trait {
            span,
            visibility: trait_decl.visibility.clone(),
            name: trait_decl.name.clone(),
            generics: self.lower_generics(&trait_decl.generics, span),
            methods: trait_decl
                .methods
                .iter()
                .map(|method| self.lower_function(method, span))
                .collect(),
        }
    }

    fn lower_impl(&mut self, impl_block: &ast::ImplBlock, span: Span) -> Impl {
        Impl {
            span,
            generics: self.lower_generics(&impl_block.generics, span),
            trait_ty: impl_block
                .trait_ty
                .as_ref()
                .map(|ty| self.lower_type_expr(ty)),
            target: self.lower_type_expr(&impl_block.target),
            where_clause: self.lower_where_clause(&impl_block.where_clause),
            methods: impl_block
                .methods
                .iter()
                .map(|method| self.lower_function(method, span))
                .collect(),
        }
    }

    fn lower_extend(&mut self, extend_block: &ast::ExtendBlock, span: Span) -> Extend {
        Extend {
            span,
            target: self.lower_type_expr(&extend_block.target),
            methods: extend_block
                .methods
                .iter()
                .map(|method| self.lower_function(method, span))
                .collect(),
        }
    }

    fn lower_type_alias(&mut self, alias: &ast::TypeAliasDecl, span: Span) -> TypeAlias {
        TypeAlias {
            span,
            visibility: alias.visibility.clone(),
            is_opaque: alias.is_opaque,
            name: alias.name.clone(),
            generics: self.lower_generics(&alias.generics, span),
            ty: self.lower_type_expr(&alias.ty),
        }
    }

    fn lower_extern_block(&mut self, extern_block: &ast::ExternBlock, span: Span) -> ExternBlock {
        ExternBlock {
            span,
            visibility: extern_block.visibility.clone(),
            abi: extern_block.abi.clone(),
            functions: extern_block
                .functions
                .iter()
                .map(|function| self.lower_function(function, span))
                .collect(),
        }
    }

    fn lower_type_expr(&mut self, ty: &ast::TypeExpr) -> TypeId {
        let kind = match &ty.kind {
            ast::TypeExprKind::Pointer { is_const, inner } => TypeKind::Pointer {
                is_const: *is_const,
                inner: self.lower_type_expr(inner),
            },
            ast::TypeExprKind::Named { path, args } => TypeKind::Named {
                path: path.clone(),
                args: args.iter().map(|arg| self.lower_type_expr(arg)).collect(),
            },
            ast::TypeExprKind::Tuple(items) => TypeKind::Tuple(
                items
                    .iter()
                    .map(|item| self.lower_type_expr(item))
                    .collect(),
            ),
            ast::TypeExprKind::Callable { params, ret } => TypeKind::Callable {
                params: params
                    .iter()
                    .map(|param| self.lower_type_expr(param))
                    .collect(),
                ret: self.lower_type_expr(ret),
            },
        };

        self.module.alloc_type(Type {
            span: ty.span,
            kind,
        })
    }

    fn lower_block(&mut self, block: &ast::Block) -> crate::BlockId {
        let statements = block
            .statements
            .iter()
            .map(|stmt| self.lower_stmt(stmt))
            .collect();
        let tail = block.tail.as_ref().map(|expr| self.lower_expr(expr));

        self.module.alloc_block(Block {
            span: block.span,
            statements,
            tail,
        })
    }

    fn lower_stmt(&mut self, stmt: &ast::Stmt) -> crate::StmtId {
        let kind = match &stmt.kind {
            ast::StmtKind::Let {
                mutable,
                pattern,
                value,
            } => StmtKind::Let {
                mutable: *mutable,
                pattern: self.lower_pattern(pattern),
                value: self.lower_expr(value),
            },
            ast::StmtKind::Return(expr) => {
                StmtKind::Return(expr.as_ref().map(|expr| self.lower_expr(expr)))
            }
            ast::StmtKind::Defer(expr) => StmtKind::Defer(self.lower_expr(expr)),
            ast::StmtKind::Break => StmtKind::Break,
            ast::StmtKind::Continue => StmtKind::Continue,
            ast::StmtKind::While { condition, body } => StmtKind::While {
                condition: self.lower_expr(condition),
                body: self.lower_block(body),
            },
            ast::StmtKind::Loop { body } => StmtKind::Loop {
                body: self.lower_block(body),
            },
            ast::StmtKind::For {
                is_await,
                pattern,
                iterable,
                body,
            } => StmtKind::For {
                is_await: *is_await,
                pattern: self.lower_pattern(pattern),
                iterable: self.lower_expr(iterable),
                body: self.lower_block(body),
            },
            ast::StmtKind::Expr { expr, terminated } => StmtKind::Expr {
                expr: self.lower_expr(expr),
                terminated: *terminated,
            },
        };

        self.module.alloc_stmt(Stmt {
            span: stmt.span,
            kind,
        })
    }

    fn lower_pattern(&mut self, pattern: &ast::Pattern) -> crate::PatternId {
        let kind = match &pattern.kind {
            ast::PatternKind::Name(name) => {
                let local = self.module.alloc_local(Local {
                    name: name.clone(),
                    span: pattern.span,
                });
                PatternKind::Binding(local)
            }
            ast::PatternKind::Tuple(items) => {
                PatternKind::Tuple(items.iter().map(|item| self.lower_pattern(item)).collect())
            }
            ast::PatternKind::Path(path) => PatternKind::Path(path.clone()),
            ast::PatternKind::TupleStruct { path, items } => PatternKind::TupleStruct {
                path: path.clone(),
                items: items.iter().map(|item| self.lower_pattern(item)).collect(),
            },
            ast::PatternKind::Struct {
                path,
                fields,
                has_rest,
            } => PatternKind::Struct {
                path: path.clone(),
                fields: fields
                    .iter()
                    .map(|field| PatternField {
                        name: field.name.clone(),
                        span: prefer_span(
                            field
                                .pattern
                                .as_ref()
                                .map(|pattern| pattern.span)
                                .unwrap_or(pattern.span),
                            pattern.span,
                        ),
                        pattern: field
                            .pattern
                            .as_ref()
                            .map(|pattern| self.lower_pattern(pattern)),
                    })
                    .collect(),
                has_rest: *has_rest,
            },
            ast::PatternKind::Integer(value) => PatternKind::Integer(value.clone()),
            ast::PatternKind::String(value) => PatternKind::String(value.clone()),
            ast::PatternKind::Bool(value) => PatternKind::Bool(*value),
            ast::PatternKind::NoneLiteral => PatternKind::NoneLiteral,
            ast::PatternKind::Wildcard => PatternKind::Wildcard,
        };

        self.module.alloc_pattern(Pattern {
            span: pattern.span,
            kind,
        })
    }

    fn lower_expr(&mut self, expr: &ast::Expr) -> crate::ExprId {
        let kind = match &expr.kind {
            ast::ExprKind::Name(name) => ExprKind::Name(name.clone()),
            ast::ExprKind::Integer(value) => ExprKind::Integer(value.clone()),
            ast::ExprKind::String { value, is_format } => ExprKind::String {
                value: value.clone(),
                is_format: *is_format,
            },
            ast::ExprKind::Bool(value) => ExprKind::Bool(*value),
            ast::ExprKind::NoneLiteral => ExprKind::NoneLiteral,
            ast::ExprKind::Tuple(items) => {
                ExprKind::Tuple(items.iter().map(|item| self.lower_expr(item)).collect())
            }
            ast::ExprKind::Array(items) => {
                ExprKind::Array(items.iter().map(|item| self.lower_expr(item)).collect())
            }
            ast::ExprKind::Block(block) => ExprKind::Block(self.lower_block(block)),
            ast::ExprKind::Unsafe(block) => ExprKind::Unsafe(self.lower_block(block)),
            ast::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => ExprKind::If {
                condition: self.lower_expr(condition),
                then_branch: self.lower_block(then_branch),
                else_branch: else_branch.as_ref().map(|expr| self.lower_expr(expr)),
            },
            ast::ExprKind::Match { value, arms } => ExprKind::Match {
                value: self.lower_expr(value),
                arms: arms.iter().map(|arm| self.lower_match_arm(arm)).collect(),
            },
            ast::ExprKind::Closure {
                is_move,
                params,
                body,
            } => ExprKind::Closure {
                is_move: *is_move,
                params: params
                    .iter()
                    .map(|name| {
                        self.module.alloc_local(Local {
                            name: name.clone(),
                            span: expr.span,
                        })
                    })
                    .collect(),
                body: self.lower_expr(body),
            },
            ast::ExprKind::Call { callee, args } => ExprKind::Call {
                callee: self.lower_expr(callee),
                args: args.iter().map(|arg| self.lower_call_arg(arg)).collect(),
            },
            ast::ExprKind::Member { object, field } => ExprKind::Member {
                object: self.lower_expr(object),
                field: field.clone(),
            },
            ast::ExprKind::Bracket { target, items } => ExprKind::Bracket {
                target: self.lower_expr(target),
                items: items.iter().map(|item| self.lower_expr(item)).collect(),
            },
            ast::ExprKind::StructLiteral { path, fields } => ExprKind::StructLiteral {
                path: path.clone(),
                fields: fields
                    .iter()
                    .map(|field| StructLiteralField {
                        name: field.name.clone(),
                        span: prefer_span(
                            field
                                .value
                                .as_ref()
                                .map(|value| value.span)
                                .unwrap_or(expr.span),
                            expr.span,
                        ),
                        value: field.value.as_ref().map(|value| self.lower_expr(value)),
                    })
                    .collect(),
            },
            ast::ExprKind::Binary { left, op, right } => ExprKind::Binary {
                left: self.lower_expr(left),
                op: *op,
                right: self.lower_expr(right),
            },
            ast::ExprKind::Unary { op, expr } => ExprKind::Unary {
                op: *op,
                expr: self.lower_expr(expr),
            },
            ast::ExprKind::Question(expr) => ExprKind::Question(self.lower_expr(expr)),
        };

        self.module.alloc_expr(Expr {
            span: expr.span,
            kind,
        })
    }

    fn lower_match_arm(&mut self, arm: &ast::MatchArm) -> MatchArm {
        MatchArm {
            pattern: self.lower_pattern(&arm.pattern),
            guard: arm.guard.as_ref().map(|expr| self.lower_expr(expr)),
            body: self.lower_expr(&arm.body),
        }
    }

    fn lower_call_arg(&mut self, arg: &ast::CallArg) -> CallArg {
        match arg {
            ast::CallArg::Positional(expr) => CallArg::Positional(self.lower_expr(expr)),
            ast::CallArg::Named { name, value } => CallArg::Named {
                name: name.clone(),
                value: self.lower_expr(value),
            },
        }
    }
}

fn prefer_span(candidate: Span, fallback: Span) -> Span {
    if candidate.is_empty() {
        fallback
    } else {
        candidate
    }
}

#[cfg(test)]
mod tests {
    use ql_parser::parse_source;

    use crate::{ExprKind, ItemKind, PatternKind, StmtKind, lower_module};

    #[test]
    fn lower_module_tracks_pattern_bindings_as_locals() {
        let source = r#"
fn main() {
    let (left, right) = pair;
}
"#;
        let ast = parse_source(source).expect("source should parse");
        let hir = lower_module(&ast);

        let function = match &hir.item(hir.items[0]).kind {
            ItemKind::Function(function) => function,
            other => panic!("expected function item, got {other:?}"),
        };
        let body = hir.block(function.body.expect("function should have body"));
        let stmt = hir.stmt(body.statements[0]);
        let pattern_id = match &stmt.kind {
            StmtKind::Let { pattern, .. } => *pattern,
            other => panic!("expected let statement, got {other:?}"),
        };
        let pattern = hir.pattern(pattern_id);

        let PatternKind::Tuple(items) = &pattern.kind else {
            panic!("expected tuple pattern");
        };

        let left = hir.pattern(items[0]);
        let right = hir.pattern(items[1]);
        let left_local = match left.kind {
            PatternKind::Binding(local) => local,
            _ => panic!("expected left binding"),
        };
        let right_local = match right.kind {
            PatternKind::Binding(local) => local,
            _ => panic!("expected right binding"),
        };

        assert_eq!(hir.local(left_local).name, "left");
        assert_eq!(hir.local(right_local).name, "right");
        assert_eq!(hir.locals().len(), 2);
    }

    #[test]
    fn lower_module_preserves_struct_literal_fields() {
        let source = r#"
fn main() {
    Point { x: 1, y: 2 };
}
"#;
        let ast = parse_source(source).expect("source should parse");
        let hir = lower_module(&ast);

        let function = match &hir.item(hir.items[0]).kind {
            ItemKind::Function(function) => function,
            other => panic!("expected function item, got {other:?}"),
        };
        let body = hir.block(function.body.expect("function should have body"));
        let stmt = hir.stmt(body.statements[0]);
        let expr_id = match &stmt.kind {
            StmtKind::Expr { expr, .. } => *expr,
            other => panic!("expected expression statement, got {other:?}"),
        };
        let expr = hir.expr(expr_id);

        let ExprKind::StructLiteral { fields, .. } = &expr.kind else {
            panic!("expected struct literal");
        };

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "x");
        assert_eq!(fields[1].name, "y");
    }
}
