use std::collections::HashMap;

use ql_ast::Path;
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{
    BlockId, CallArg, EnumVariant, ExprId, ExprKind, Field, Function, FunctionRef, GenericParam,
    Global, ItemId, ItemKind, MatchArm, Module, Param, PatternId, PatternKind, StmtKind,
    StructLiteralField, TypeId, TypeKind, VariantFields, WherePredicate,
};

use crate::{
    BuiltinType, GenericBinding, ImportBinding, NamedTypeBinding, NamedValueBinding, ParamBinding,
    ResolutionMap, Scope, ScopeGraph, ScopeId, ScopeKind, TypeResolution, ValueResolution,
};

pub fn resolve_module(module: &Module) -> ResolutionMap {
    let mut resolver = Resolver::new(module);
    resolver.resolve();
    resolver.finish()
}

#[derive(Default)]
struct LookupScope {
    values: HashMap<String, ValueResolution>,
    types: HashMap<String, TypeResolution>,
}

struct Resolver<'module> {
    module: &'module Module,
    resolution: ResolutionMap,
    lookups: Vec<LookupScope>,
    module_scope: ScopeId,
}

impl<'module> Resolver<'module> {
    fn new(module: &'module Module) -> Self {
        let mut resolution = ResolutionMap::default();
        let module_scope = resolution.scopes.push(Scope::new(ScopeKind::Module, None));
        Self {
            module,
            resolution,
            lookups: vec![LookupScope::default()],
            module_scope,
        }
    }

    fn finish(self) -> ResolutionMap {
        self.resolution
    }

    fn resolve(&mut self) {
        self.seed_builtins();
        self.seed_imports();
        self.seed_top_level_items();

        for &item_id in &self.module.items {
            self.resolve_item(item_id, self.module_scope);
        }
    }

    fn seed_builtins(&mut self) {
        for (name, builtin) in builtin_types() {
            self.bind_type(
                self.module_scope,
                (*name).to_owned(),
                TypeResolution::Builtin(*builtin),
            );
        }
    }

    fn seed_imports(&mut self) {
        for use_decl in &self.module.uses {
            if let Some(group) = &use_decl.group {
                for item in group {
                    self.bind_import(
                        self.module_scope,
                        ImportBinding::grouped(&use_decl.prefix, item),
                    );
                }
            } else {
                self.bind_import(self.module_scope, ImportBinding::direct(use_decl));
            }
        }
    }

    fn seed_top_level_items(&mut self) {
        for &item_id in &self.module.items {
            match &self.module.item(item_id).kind {
                ItemKind::Function(function) => {
                    self.bind_value(
                        self.module_scope,
                        function.name.clone(),
                        ValueResolution::Function(FunctionRef::Item(item_id)),
                    );
                }
                ItemKind::Const(global) | ItemKind::Static(global) => {
                    self.bind_value(
                        self.module_scope,
                        global.name.clone(),
                        ValueResolution::Item(item_id),
                    );
                }
                ItemKind::Struct(struct_decl) => {
                    self.bind_type(
                        self.module_scope,
                        struct_decl.name.clone(),
                        TypeResolution::Item(item_id),
                    );
                    self.bind_value(
                        self.module_scope,
                        struct_decl.name.clone(),
                        ValueResolution::Item(item_id),
                    );
                }
                ItemKind::Enum(enum_decl) => {
                    self.bind_type(
                        self.module_scope,
                        enum_decl.name.clone(),
                        TypeResolution::Item(item_id),
                    );
                    self.bind_value(
                        self.module_scope,
                        enum_decl.name.clone(),
                        ValueResolution::Item(item_id),
                    );
                }
                ItemKind::Trait(trait_decl) => {
                    self.bind_type(
                        self.module_scope,
                        trait_decl.name.clone(),
                        TypeResolution::Item(item_id),
                    );
                }
                ItemKind::TypeAlias(alias) => {
                    self.bind_type(
                        self.module_scope,
                        alias.name.clone(),
                        TypeResolution::Item(item_id),
                    );
                }
                ItemKind::ExternBlock(extern_block) => {
                    for (index, function) in extern_block.functions.iter().enumerate() {
                        self.bind_value(
                            self.module_scope,
                            function.name.clone(),
                            ValueResolution::Function(FunctionRef::ExternBlockMember {
                                block: item_id,
                                index,
                            }),
                        );
                    }
                }
                ItemKind::Impl(_) | ItemKind::Extend(_) => {}
            }
        }
    }

    fn resolve_item(&mut self, item_id: ItemId, parent_scope: ScopeId) {
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => {
                let scope = self.resolve_function(function, parent_scope);
                self.resolution.item_scopes.insert(item_id, scope);
            }
            ItemKind::Const(global) | ItemKind::Static(global) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.resolve_global(global, scope);
            }
            ItemKind::Struct(struct_decl) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.bind_generics(scope, &struct_decl.generics);
                self.resolve_fields(&struct_decl.fields, scope);
            }
            ItemKind::Enum(enum_decl) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.bind_generics(scope, &enum_decl.generics);
                for variant in &enum_decl.variants {
                    self.resolve_variant(variant, scope);
                }
            }
            ItemKind::Trait(trait_decl) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.bind_generics(scope, &trait_decl.generics);
                for method in &trait_decl.methods {
                    self.resolve_function(method, scope);
                }
            }
            ItemKind::Impl(impl_block) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.bind_generics(scope, &impl_block.generics);
                if let Some(trait_ty) = impl_block.trait_ty {
                    self.resolve_type(trait_ty, scope);
                }
                self.resolve_type(impl_block.target, scope);
                self.resolve_where_clause(&impl_block.where_clause, scope);
                for method in &impl_block.methods {
                    self.resolve_function(method, scope);
                }
            }
            ItemKind::Extend(extend_block) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.resolve_type(extend_block.target, scope);
                for method in &extend_block.methods {
                    self.resolve_function(method, scope);
                }
            }
            ItemKind::TypeAlias(alias) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                self.bind_generics(scope, &alias.generics);
                self.resolve_type(alias.ty, scope);
            }
            ItemKind::ExternBlock(extern_block) => {
                let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
                self.resolution.item_scopes.insert(item_id, scope);
                for function in &extern_block.functions {
                    self.resolve_function(function, scope);
                }
            }
        }
    }

    fn resolve_global(&mut self, global: &Global, scope: ScopeId) {
        self.resolve_type(global.ty, scope);
        self.resolve_expr(global.value, scope);
    }

    fn resolve_fields(&mut self, fields: &[Field], scope: ScopeId) {
        for field in fields {
            self.resolve_type(field.ty, scope);
            if let Some(default) = field.default {
                self.resolve_expr(default, scope);
            }
        }
    }

    fn resolve_variant(&mut self, variant: &EnumVariant, scope: ScopeId) {
        match &variant.fields {
            VariantFields::Unit => {}
            VariantFields::Tuple(types) => {
                for &type_id in types {
                    self.resolve_type(type_id, scope);
                }
            }
            VariantFields::Struct(fields) => {
                self.resolve_fields(fields, scope);
            }
        }
    }

    fn resolve_function(&mut self, function: &Function, parent_scope: ScopeId) -> ScopeId {
        let scope = self.alloc_scope(ScopeKind::Item, Some(parent_scope));
        self.resolution.function_scopes.insert(function.span, scope);
        self.bind_generics(scope, &function.generics);
        self.bind_params(scope, &function.params);

        for param in &function.params {
            if let Param::Regular(param) = param {
                self.resolve_type(param.ty, scope);
            }
        }

        if let Some(return_type) = function.return_type {
            self.resolve_type(return_type, scope);
        }

        self.resolve_where_clause(&function.where_clause, scope);

        if let Some(body) = function.body {
            self.resolve_block(body, scope);
        }

        scope
    }

    fn resolve_where_clause(&mut self, predicates: &[WherePredicate], scope: ScopeId) {
        for predicate in predicates {
            self.resolve_type(predicate.target, scope);
        }
    }

    fn bind_generics(&mut self, scope: ScopeId, generics: &[GenericParam]) {
        for (index, generic) in generics.iter().enumerate() {
            self.bind_type(
                scope,
                generic.name.clone(),
                TypeResolution::Generic(GenericBinding { scope, index }),
            );
        }
    }

    fn bind_params(&mut self, scope: ScopeId, params: &[Param]) {
        for (index, param) in params.iter().enumerate() {
            match param {
                Param::Regular(param) => self.bind_value(
                    scope,
                    param.name.clone(),
                    ValueResolution::Param(ParamBinding { scope, index }),
                ),
                Param::Receiver(_) => {
                    self.bind_value(scope, "self".to_owned(), ValueResolution::SelfValue);
                }
            }
        }
    }

    fn resolve_type(&mut self, type_id: TypeId, scope: ScopeId) {
        self.resolution.type_scopes.insert(type_id, scope);

        let ty = self.module.ty(type_id);
        match &ty.kind {
            TypeKind::Pointer { inner, .. } => self.resolve_type(*inner, scope),
            TypeKind::Named { path, args } => {
                if let Some(resolution) = self.lookup_type_path(path, scope) {
                    self.resolution.type_paths.insert(type_id, resolution);
                }
                for &arg in args {
                    self.resolve_type(arg, scope);
                }
            }
            TypeKind::Tuple(items) => {
                for &item in items {
                    self.resolve_type(item, scope);
                }
            }
            TypeKind::Callable { params, ret } => {
                for &param in params {
                    self.resolve_type(param, scope);
                }
                self.resolve_type(*ret, scope);
            }
        }
    }

    fn resolve_block(&mut self, block_id: BlockId, parent_scope: ScopeId) {
        let scope = self.alloc_scope(ScopeKind::Block, Some(parent_scope));
        self.resolution.block_scopes.insert(block_id, scope);

        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                StmtKind::Let { pattern, value, .. } => {
                    self.resolve_pattern(*pattern, scope);
                    self.resolve_expr(*value, scope);
                    self.bind_pattern_locals(*pattern, scope);
                }
                StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.resolve_expr(*expr, scope);
                    }
                }
                StmtKind::Defer(expr) => self.resolve_expr(*expr, scope),
                StmtKind::Break | StmtKind::Continue => {}
                StmtKind::While { condition, body } => {
                    self.resolve_expr(*condition, scope);
                    self.resolve_block(*body, scope);
                }
                StmtKind::Loop { body } => self.resolve_block(*body, scope),
                StmtKind::For {
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    self.resolve_expr(*iterable, scope);
                    let loop_scope = self.alloc_scope(ScopeKind::ForLoop, Some(scope));
                    self.resolve_pattern(*pattern, loop_scope);
                    self.bind_pattern_locals(*pattern, loop_scope);
                    self.resolve_block(*body, loop_scope);
                }
                StmtKind::Expr { expr, .. } => self.resolve_expr(*expr, scope),
            }
        }

        if let Some(expr) = block.tail {
            self.resolve_expr(expr, scope);
        }
    }

    fn resolve_pattern(&mut self, pattern_id: PatternId, scope: ScopeId) {
        self.resolution.pattern_scopes.insert(pattern_id, scope);

        let pattern = self.module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(_) => {}
            PatternKind::Tuple(items) => {
                for &item in items {
                    self.resolve_pattern(item, scope);
                }
            }
            PatternKind::Path(path) => {
                if let Some(resolution) = self.lookup_value_path(path, scope) {
                    self.resolution.pattern_paths.insert(pattern_id, resolution);
                }
            }
            PatternKind::TupleStruct { path, items } => {
                if let Some(resolution) = self.lookup_value_path(path, scope) {
                    self.resolution.pattern_paths.insert(pattern_id, resolution);
                }
                for &item in items {
                    self.resolve_pattern(item, scope);
                }
            }
            PatternKind::Struct { path, fields, .. } => {
                if let Some(resolution) = self.lookup_value_path(path, scope) {
                    self.resolution.pattern_paths.insert(pattern_id, resolution);
                }
                for field in fields {
                    self.resolve_pattern(field.pattern, scope);
                }
            }
            PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
    }

    fn bind_pattern_locals(&mut self, pattern_id: PatternId, scope: ScopeId) {
        let pattern = self.module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(local_id) => {
                let local = self.module.local(*local_id);
                self.bind_value(scope, local.name.clone(), ValueResolution::Local(*local_id));
            }
            PatternKind::Tuple(items) | PatternKind::TupleStruct { items, .. } => {
                for &item in items {
                    self.bind_pattern_locals(item, scope);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for field in fields {
                    self.bind_pattern_locals(field.pattern, scope);
                }
            }
            PatternKind::Path(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
    }

    fn resolve_expr(&mut self, expr_id: ExprId, scope: ScopeId) {
        self.resolution.expr_scopes.insert(expr_id, scope);

        let expr = self.module.expr(expr_id);
        match &expr.kind {
            ExprKind::Name(name) => match self.lookup_value_name(name, scope) {
                Some(resolution) => {
                    self.resolution.value_paths.insert(expr_id, resolution);
                }
                None if name == "self" => self
                    .resolution
                    .diagnostics
                    .push(invalid_self_diagnostic(expr.span)),
                None => {}
            },
            ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.resolve_expr(item, scope);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                self.resolve_block(*block_id, scope);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(*condition, scope);
                self.resolve_block(*then_branch, scope);
                if let Some(expr) = else_branch {
                    self.resolve_expr(*expr, scope);
                }
            }
            ExprKind::Match { value, arms } => {
                self.resolve_expr(*value, scope);
                for arm in arms {
                    self.resolve_match_arm(arm, scope);
                }
            }
            ExprKind::Closure { params, body, .. } => {
                let closure_scope = self.alloc_scope(ScopeKind::Closure, Some(scope));
                for &local_id in params {
                    let local = self.module.local(local_id);
                    self.bind_value(
                        closure_scope,
                        local.name.clone(),
                        ValueResolution::Local(local_id),
                    );
                }
                self.resolve_expr(*body, closure_scope);
            }
            ExprKind::Call { callee, args } => {
                self.resolve_expr(*callee, scope);
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => self.resolve_expr(*expr, scope),
                        CallArg::Named { value, .. } => self.resolve_expr(*value, scope),
                    }
                }
            }
            ExprKind::Member { object, .. } => {
                self.resolve_expr(*object, scope);
                if let Some(path) = self.expr_path(expr_id)
                    && let Some(resolution) = self.lookup_value_path(&path, scope)
                {
                    self.resolution.value_paths.insert(expr_id, resolution);
                }
            }
            ExprKind::Bracket { target, items } => {
                self.resolve_expr(*target, scope);
                for &item in items {
                    self.resolve_expr(item, scope);
                }
            }
            ExprKind::StructLiteral { path, fields } => {
                if let Some(resolution) = self.lookup_type_path(path, scope) {
                    self.resolution
                        .struct_literal_paths
                        .insert(expr_id, resolution);
                }
                self.resolve_struct_literal_fields(fields, scope);
            }
            ExprKind::Binary { left, right, .. } => {
                self.resolve_expr(*left, scope);
                self.resolve_expr(*right, scope);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
                self.resolve_expr(*expr, scope);
            }
        }
    }

    fn resolve_match_arm(&mut self, arm: &MatchArm, scope: ScopeId) {
        let arm_scope = self.alloc_scope(ScopeKind::MatchArm, Some(scope));
        self.resolve_pattern(arm.pattern, arm_scope);
        self.bind_pattern_locals(arm.pattern, arm_scope);
        if let Some(guard) = arm.guard {
            self.resolve_expr(guard, arm_scope);
        }
        self.resolve_expr(arm.body, arm_scope);
    }

    fn resolve_struct_literal_fields(&mut self, fields: &[StructLiteralField], scope: ScopeId) {
        for field in fields {
            self.resolve_expr(field.value, scope);
        }
    }

    fn expr_path(&self, expr_id: ExprId) -> Option<Path> {
        let expr = self.module.expr(expr_id);
        match &expr.kind {
            ExprKind::Name(name) => Some(Path::with_spans(vec![name.clone()], vec![expr.span])),
            ExprKind::Member {
                object,
                field,
                field_span,
            } => {
                let mut path = self.expr_path(*object)?;
                path.segments.push(field.clone());
                path.segment_spans.push(*field_span);
                Some(path)
            }
            _ => None,
        }
    }

    fn lookup_value_path(&self, path: &Path, scope: ScopeId) -> Option<ValueResolution> {
        path.segments
            .first()
            .and_then(|name| self.lookup_value_name(name, scope))
    }

    fn lookup_type_path(&self, path: &Path, scope: ScopeId) -> Option<TypeResolution> {
        path.segments
            .first()
            .and_then(|name| self.lookup_type_name(name, scope))
    }

    fn lookup_value_name(&self, name: &str, scope: ScopeId) -> Option<ValueResolution> {
        self.lookup_scopes(scope)
            .find_map(|scope_id| self.lookups[scope_id.index()].values.get(name).cloned())
    }

    fn lookup_type_name(&self, name: &str, scope: ScopeId) -> Option<TypeResolution> {
        self.lookup_scopes(scope)
            .find_map(|scope_id| self.lookups[scope_id.index()].types.get(name).cloned())
    }

    fn lookup_scopes(&self, scope: ScopeId) -> ScopeWalk<'_> {
        ScopeWalk {
            scopes: &self.resolution.scopes,
            next: Some(scope),
        }
    }

    fn alloc_scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        let scope_id = self.resolution.scopes.push(Scope::new(kind, parent));
        self.lookups.push(LookupScope::default());
        scope_id
    }

    fn bind_import(&mut self, scope: ScopeId, binding: ImportBinding) {
        self.bind_value(
            scope,
            binding.local_name.clone(),
            ValueResolution::Import(binding.clone()),
        );
        self.bind_type(
            scope,
            binding.local_name.clone(),
            TypeResolution::Import(binding),
        );
    }

    fn bind_value(&mut self, scope: ScopeId, name: String, resolution: ValueResolution) {
        self.lookups[scope.index()]
            .values
            .insert(name.clone(), resolution.clone());
        self.resolution
            .scopes
            .scope_mut(scope)
            .value_bindings
            .push(NamedValueBinding { name, resolution });
    }

    fn bind_type(&mut self, scope: ScopeId, name: String, resolution: TypeResolution) {
        self.lookups[scope.index()]
            .types
            .insert(name.clone(), resolution.clone());
        self.resolution
            .scopes
            .scope_mut(scope)
            .type_bindings
            .push(NamedTypeBinding { name, resolution });
    }
}

struct ScopeWalk<'graph> {
    scopes: &'graph ScopeGraph,
    next: Option<ScopeId>,
}

impl Iterator for ScopeWalk<'_> {
    type Item = ScopeId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = self.scopes.scope(current).parent;
        Some(current)
    }
}

fn builtin_types() -> &'static [(&'static str, BuiltinType)] {
    &[
        ("Bool", BuiltinType::Bool),
        ("Char", BuiltinType::Char),
        ("String", BuiltinType::String),
        ("Bytes", BuiltinType::Bytes),
        ("Void", BuiltinType::Void),
        ("Never", BuiltinType::Never),
        ("Int", BuiltinType::Int),
        ("UInt", BuiltinType::UInt),
        ("I8", BuiltinType::I8),
        ("I16", BuiltinType::I16),
        ("I32", BuiltinType::I32),
        ("I64", BuiltinType::I64),
        ("ISize", BuiltinType::ISize),
        ("U8", BuiltinType::U8),
        ("U16", BuiltinType::U16),
        ("U32", BuiltinType::U32),
        ("U64", BuiltinType::U64),
        ("USize", BuiltinType::USize),
        ("F32", BuiltinType::F32),
        ("F64", BuiltinType::F64),
    ]
}

fn invalid_self_diagnostic(span: ql_span::Span) -> Diagnostic {
    Diagnostic::error("invalid use of `self` outside a method receiver scope")
        .with_label(Label::new(span).with_message("`self` is only available inside methods"))
}
