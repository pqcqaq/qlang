use std::collections::{HashMap, hash_map::Entry};

use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{
    BlockId, CallArg, ExprId, ExprKind, Field, Function, GenericParam, ItemKind, LocalId, Module,
    Param, PatternField, PatternId, PatternKind, StmtKind, StructLiteralField, VariantFields,
};
use ql_span::Span;

/// Run the Phase 2 bootstrap semantic checks over lowered HIR.
pub fn check_module(module: &Module) -> Vec<Diagnostic> {
    let mut checker = Checker::default();
    checker.check_module(module);
    checker.diagnostics
}

#[derive(Default)]
struct Checker {
    diagnostics: Vec<Diagnostic>,
}

impl Checker {
    fn check_module(&mut self, module: &Module) {
        self.check_top_level_names(module);

        for &item_id in &module.items {
            let item = module.item(item_id);
            match &item.kind {
                ItemKind::Function(function) => self.check_function(module, function),
                ItemKind::Const(global) | ItemKind::Static(global) => {
                    self.check_expr(module, global.value);
                }
                ItemKind::Struct(struct_decl) => {
                    self.check_generics(&struct_decl.generics);
                    self.check_named_fields("struct", &struct_decl.fields);
                    for field in &struct_decl.fields {
                        if let Some(default) = field.default {
                            self.check_expr(module, default);
                        }
                    }
                }
                ItemKind::Enum(enum_decl) => {
                    self.check_generics(&enum_decl.generics);
                    for variant in &enum_decl.variants {
                        if let VariantFields::Struct(fields) = &variant.fields {
                            self.check_named_fields("enum variant", fields);
                        }
                    }
                }
                ItemKind::Trait(trait_decl) => {
                    self.check_generics(&trait_decl.generics);
                    for method in &trait_decl.methods {
                        self.check_function(module, method);
                    }
                }
                ItemKind::Impl(impl_block) => {
                    self.check_generics(&impl_block.generics);
                    for method in &impl_block.methods {
                        self.check_function(module, method);
                    }
                }
                ItemKind::Extend(extend_block) => {
                    for method in &extend_block.methods {
                        self.check_function(module, method);
                    }
                }
                ItemKind::TypeAlias(alias) => {
                    self.check_generics(&alias.generics);
                }
                ItemKind::ExternBlock(extern_block) => {
                    for function in &extern_block.functions {
                        self.check_function(module, function);
                    }
                }
            }
        }
    }

    fn check_top_level_names(&mut self, module: &Module) {
        let mut seen = HashMap::<String, Span>::new();

        for &item_id in &module.items {
            let item = module.item(item_id);
            match &item.kind {
                ItemKind::Function(function) => {
                    self.record_named_span(
                        &mut seen,
                        "duplicate top-level definition",
                        &function.name,
                        function.span,
                    );
                }
                ItemKind::Const(global) | ItemKind::Static(global) => {
                    self.record_named_span(
                        &mut seen,
                        "duplicate top-level definition",
                        &global.name,
                        global.span,
                    );
                }
                ItemKind::Struct(struct_decl) => {
                    self.record_named_span(
                        &mut seen,
                        "duplicate top-level definition",
                        &struct_decl.name,
                        struct_decl.span,
                    );
                }
                ItemKind::Enum(enum_decl) => {
                    self.record_named_span(
                        &mut seen,
                        "duplicate top-level definition",
                        &enum_decl.name,
                        enum_decl.span,
                    );
                }
                ItemKind::Trait(trait_decl) => {
                    self.record_named_span(
                        &mut seen,
                        "duplicate top-level definition",
                        &trait_decl.name,
                        trait_decl.span,
                    );
                }
                ItemKind::TypeAlias(alias) => {
                    self.record_named_span(
                        &mut seen,
                        "duplicate top-level definition",
                        &alias.name,
                        alias.span,
                    );
                }
                ItemKind::ExternBlock(extern_block) => {
                    for function in &extern_block.functions {
                        self.record_named_span(
                            &mut seen,
                            "duplicate top-level definition",
                            &function.name,
                            function.span,
                        );
                    }
                }
                ItemKind::Impl(_) | ItemKind::Extend(_) => {}
            }
        }
    }

    fn check_function(&mut self, module: &Module, function: &Function) {
        self.check_generics(&function.generics);
        self.check_params(&function.params);

        if let Some(body) = function.body {
            self.check_block(module, body);
        }
    }

    fn check_generics(&mut self, generics: &[GenericParam]) {
        let mut seen = HashMap::<String, Span>::new();
        for generic in generics {
            self.record_named_span(
                &mut seen,
                "duplicate generic parameter",
                &generic.name,
                generic.span,
            );
        }
    }

    fn check_params(&mut self, params: &[Param]) {
        let mut seen = HashMap::<String, Span>::new();
        for param in params {
            if let Param::Regular(param) = param {
                self.record_named_span(&mut seen, "duplicate parameter", &param.name, param.span);
            }
        }
    }

    fn check_named_fields(&mut self, container: &str, fields: &[Field]) {
        let mut seen = HashMap::<String, Span>::new();
        for field in fields {
            self.record_named_span(
                &mut seen,
                &format!("duplicate field in {container}"),
                &field.name,
                field.span,
            );
        }
    }

    fn check_block(&mut self, module: &Module, block_id: BlockId) {
        let block = module.block(block_id);

        for &stmt_id in &block.statements {
            let stmt = module.stmt(stmt_id);
            match &stmt.kind {
                StmtKind::Let { pattern, value, .. } => {
                    self.check_binding_pattern(module, *pattern);
                    self.check_expr(module, *value);
                }
                StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.check_expr(module, *expr);
                    }
                }
                StmtKind::Defer(expr) => self.check_expr(module, *expr),
                StmtKind::Break | StmtKind::Continue => {}
                StmtKind::While { condition, body } => {
                    self.check_expr(module, *condition);
                    self.check_block(module, *body);
                }
                StmtKind::Loop { body } => self.check_block(module, *body),
                StmtKind::For {
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    self.check_binding_pattern(module, *pattern);
                    self.check_expr(module, *iterable);
                    self.check_block(module, *body);
                }
                StmtKind::Expr { expr, .. } => self.check_expr(module, *expr),
            }
        }

        if let Some(expr) = block.tail {
            self.check_expr(module, expr);
        }
    }

    fn check_binding_pattern(&mut self, module: &Module, pattern_id: PatternId) {
        self.check_pattern_structure(module, pattern_id);

        let mut seen = HashMap::<String, Span>::new();
        self.collect_pattern_bindings(module, pattern_id, &mut seen);
    }

    fn check_pattern_structure(&mut self, module: &Module, pattern_id: PatternId) {
        let pattern = module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Tuple(items) | PatternKind::TupleStruct { items, .. } => {
                for &item in items {
                    self.check_pattern_structure(module, item);
                }
            }
            PatternKind::Struct { fields, .. } => {
                self.check_pattern_fields(fields);
                for field in fields {
                    if let Some(pattern) = field.pattern {
                        self.check_pattern_structure(module, pattern);
                    }
                }
            }
            PatternKind::Binding(_)
            | PatternKind::Path(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
    }

    fn check_pattern_fields(&mut self, fields: &[PatternField]) {
        let mut seen = HashMap::<String, Span>::new();
        for field in fields {
            self.record_named_span(
                &mut seen,
                "duplicate field in struct pattern",
                &field.name,
                field.span,
            );
        }
    }

    fn collect_pattern_bindings(
        &mut self,
        module: &Module,
        pattern_id: PatternId,
        seen: &mut HashMap<String, Span>,
    ) {
        let pattern = module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(local_id) => {
                let local = module.local(*local_id);
                self.record_named_span(
                    seen,
                    "duplicate binding in pattern",
                    &local.name,
                    local.span,
                );
            }
            PatternKind::Tuple(items) | PatternKind::TupleStruct { items, .. } => {
                for &item in items {
                    self.collect_pattern_bindings(module, item, seen);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for field in fields {
                    if let Some(pattern) = field.pattern {
                        self.collect_pattern_bindings(module, pattern, seen);
                    }
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

    fn check_expr(&mut self, module: &Module, expr_id: ExprId) {
        let expr = module.expr(expr_id);
        match &expr.kind {
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.check_expr(module, item);
                }
            }
            ExprKind::Block(block) | ExprKind::Unsafe(block) => self.check_block(module, *block),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_expr(module, *condition);
                self.check_block(module, *then_branch);
                if let Some(expr) = else_branch {
                    self.check_expr(module, *expr);
                }
            }
            ExprKind::Match { value, arms } => {
                self.check_expr(module, *value);
                for arm in arms {
                    self.check_binding_pattern(module, arm.pattern);
                    if let Some(guard) = arm.guard {
                        self.check_expr(module, guard);
                    }
                    self.check_expr(module, arm.body);
                }
            }
            ExprKind::Closure { params, body, .. } => {
                self.check_local_list(module, params, "duplicate closure parameter");
                self.check_expr(module, *body);
            }
            ExprKind::Call { callee, args } => {
                self.check_expr(module, *callee);
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => self.check_expr(module, *expr),
                        CallArg::Named { value, .. } => self.check_expr(module, *value),
                    }
                }
            }
            ExprKind::Member { object, .. } => self.check_expr(module, *object),
            ExprKind::Bracket { target, items } => {
                self.check_expr(module, *target);
                for &item in items {
                    self.check_expr(module, item);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                self.check_struct_literal_fields(fields);
                for field in fields {
                    if let Some(value) = field.value {
                        self.check_expr(module, value);
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.check_expr(module, *left);
                self.check_expr(module, *right);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
                self.check_expr(module, *expr)
            }
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
        }
    }

    fn check_local_list(&mut self, module: &Module, locals: &[LocalId], message: &str) {
        let mut seen = HashMap::<String, Span>::new();
        for &local_id in locals {
            let local = module.local(local_id);
            self.record_named_span(&mut seen, message, &local.name, local.span);
        }
    }

    fn check_struct_literal_fields(&mut self, fields: &[StructLiteralField]) {
        let mut seen = HashMap::<String, Span>::new();
        for field in fields {
            self.record_named_span(
                &mut seen,
                "duplicate field in struct literal",
                &field.name,
                field.span,
            );
        }
    }

    fn record_named_span(
        &mut self,
        seen: &mut HashMap<String, Span>,
        message: &str,
        name: &str,
        span: Span,
    ) {
        match seen.entry(name.to_owned()) {
            Entry::Occupied(entry) => {
                self.diagnostics.push(
                    Diagnostic::error(format!("{message} `{name}`"))
                        .with_label(Label::new(*entry.get()).with_message("first seen here"))
                        .with_label(Label::new(span).with_message("duplicate here")),
                );
            }
            Entry::Vacant(entry) => {
                entry.insert(span);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ql_parser::parse_source;

    use crate::check_module;

    fn diagnostics_for(source: &str) -> Vec<String> {
        let ast = parse_source(source).expect("source should parse");
        let hir = ql_hir::lower_module(&ast);
        check_module(&hir)
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    #[test]
    fn detects_duplicate_top_level_definitions() {
        let diagnostics = diagnostics_for(
            r#"
struct User {}
fn User() {}
"#,
        );

        assert!(diagnostics.contains(&"duplicate top-level definition `User`".to_string()));
    }

    #[test]
    fn detects_duplicate_generic_parameters() {
        let diagnostics = diagnostics_for(
            r#"
fn id[T, T](value: T) -> T {
    value
}
"#,
        );

        assert!(diagnostics.contains(&"duplicate generic parameter `T`".to_string()));
    }

    #[test]
    fn detects_duplicate_function_parameters() {
        let diagnostics = diagnostics_for(
            r#"
fn add(left: Int, left: Int) -> Int {
    left
}
"#,
        );

        assert!(diagnostics.contains(&"duplicate parameter `left`".to_string()));
    }

    #[test]
    fn detects_duplicate_pattern_bindings() {
        let diagnostics = diagnostics_for(
            r#"
fn main() {
    let (left, left) = pair;
}
"#,
        );

        assert!(diagnostics.contains(&"duplicate binding in pattern `left`".to_string()));
    }

    #[test]
    fn detects_duplicate_struct_fields() {
        let diagnostics = diagnostics_for(
            r#"
struct Point {
    x: Int,
    x: Int,
}
"#,
        );

        assert!(diagnostics.contains(&"duplicate field in struct `x`".to_string()));
    }

    #[test]
    fn detects_duplicate_struct_literal_fields() {
        let diagnostics = diagnostics_for(
            r#"
fn main() {
    Point { x: 1, x: 2 };
}
"#,
        );

        assert!(diagnostics.contains(&"duplicate field in struct literal `x`".to_string()));
    }
}
