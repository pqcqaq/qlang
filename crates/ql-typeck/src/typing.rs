use std::collections::{HashMap, HashSet};

use ql_ast::BinaryOp;
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{
    BlockId, CallArg, EnumVariant, ExprId, ExprKind, Function, ItemId, ItemKind, LocalId, MatchArm,
    Module, Param, PatternId, PatternKind, StmtId, StmtKind, VariantFields,
};
use ql_resolve::{ParamBinding, ResolutionMap, TypeResolution, ValueResolution};
use ql_span::Span;

use crate::checker::{FieldTarget, MemberTarget, MethodTarget};
use crate::types::{Ty, item_display_name, local_item_for_import_binding, lower_type, void_ty};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypingResult {
    pub diagnostics: Vec<Diagnostic>,
    pub expr_types: HashMap<ExprId, Ty>,
    pub pattern_types: HashMap<PatternId, Ty>,
    pub local_types: HashMap<LocalId, Ty>,
    pub member_targets: HashMap<ExprId, MemberTarget>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ControlFlowSummary {
    falls_through: bool,
    returns: bool,
    breaks: bool,
    continues: bool,
}

impl ControlFlowSummary {
    fn diverges() -> Self {
        Self::default()
    }

    fn normal() -> Self {
        Self {
            falls_through: true,
            ..Self::default()
        }
    }

    fn returns() -> Self {
        Self {
            returns: true,
            ..Self::default()
        }
    }

    fn breaks() -> Self {
        Self {
            breaks: true,
            ..Self::default()
        }
    }

    fn continues() -> Self {
        Self {
            continues: true,
            ..Self::default()
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            falls_through: self.falls_through || other.falls_through,
            returns: self.returns || other.returns,
            breaks: self.breaks || other.breaks,
            continues: self.continues || other.continues,
        }
    }

    fn then(self, next: Self) -> Self {
        if !self.falls_through {
            return self;
        }

        Self {
            falls_through: next.falls_through,
            returns: self.returns || next.returns,
            breaks: self.breaks || next.breaks,
            continues: self.continues || next.continues,
        }
    }

    fn guarantees_return(self) -> bool {
        self.returns && !self.falls_through && !self.breaks && !self.continues
    }
}

/// Run the first-pass typing checks over lowered HIR plus name-resolution data.
pub(crate) fn analyze_module(module: &Module, resolution: &ResolutionMap) -> TypingResult {
    let mut checker = Checker::new(module, resolution);
    checker.check_module();
    TypingResult {
        diagnostics: checker.diagnostics,
        expr_types: checker.expr_types,
        pattern_types: checker.pattern_types,
        local_types: checker.local_types,
        member_targets: checker.member_targets,
    }
}

struct Checker<'a> {
    module: &'a Module,
    resolution: &'a ResolutionMap,
    diagnostics: Vec<Diagnostic>,
    expr_types: HashMap<ExprId, Ty>,
    pattern_types: HashMap<PatternId, Ty>,
    local_types: HashMap<LocalId, Ty>,
    mutable_locals: HashSet<LocalId>,
    member_targets: HashMap<ExprId, MemberTarget>,
    param_types: HashMap<ParamBinding, Ty>,
    self_type: Option<Ty>,
    self_is_mutable: bool,
    current_return: Option<Ty>,
    in_async_function: bool,
    loop_depth: usize,
}

impl<'a> Checker<'a> {
    fn new(module: &'a Module, resolution: &'a ResolutionMap) -> Self {
        Self {
            module,
            resolution,
            diagnostics: Vec::new(),
            expr_types: HashMap::new(),
            pattern_types: HashMap::new(),
            local_types: HashMap::new(),
            mutable_locals: HashSet::new(),
            member_targets: HashMap::new(),
            param_types: HashMap::new(),
            self_type: None,
            self_is_mutable: false,
            current_return: None,
            in_async_function: false,
            loop_depth: 0,
        }
    }

    fn check_module(&mut self) {
        for &item_id in &self.module.items {
            match &self.module.item(item_id).kind {
                ItemKind::Function(function) => self.check_function(function, None),
                ItemKind::Const(global) | ItemKind::Static(global) => {
                    let expected = lower_type(self.module, self.resolution, global.ty);
                    let actual = self.check_expr(global.value, Some(&expected));
                    self.report_type_mismatch(global.value, &expected, &actual, "global value");
                }
                ItemKind::Struct(struct_decl) => {
                    for field in &struct_decl.fields {
                        if let Some(default) = field.default {
                            let expected = lower_type(self.module, self.resolution, field.ty);
                            let actual = self.check_expr(default, Some(&expected));
                            self.report_type_mismatch(
                                default,
                                &expected,
                                &actual,
                                "default field value",
                            );
                        }
                    }
                }
                ItemKind::Enum(enum_decl) => {
                    for variant in &enum_decl.variants {
                        if let VariantFields::Struct(fields) = &variant.fields {
                            for field in fields {
                                if let Some(default) = field.default {
                                    let expected =
                                        lower_type(self.module, self.resolution, field.ty);
                                    let actual = self.check_expr(default, Some(&expected));
                                    self.report_type_mismatch(
                                        default,
                                        &expected,
                                        &actual,
                                        "default enum field value",
                                    );
                                }
                            }
                        }
                    }
                }
                ItemKind::Trait(trait_decl) => {
                    for method in &trait_decl.methods {
                        self.check_function(method, None);
                    }
                }
                ItemKind::Impl(impl_block) => {
                    let self_ty = lower_type(self.module, self.resolution, impl_block.target);
                    for method in &impl_block.methods {
                        self.check_function(method, Some(self_ty.clone()));
                    }
                }
                ItemKind::Extend(extend_block) => {
                    let self_ty = lower_type(self.module, self.resolution, extend_block.target);
                    for method in &extend_block.methods {
                        self.check_function(method, Some(self_ty.clone()));
                    }
                }
                ItemKind::TypeAlias(_) | ItemKind::ExternBlock(_) => {}
            }
        }
    }

    fn check_function(&mut self, function: &Function, self_type: Option<Ty>) {
        let old_self = self.self_type.clone();
        let old_self_is_mutable = self.self_is_mutable;
        let old_return = self.current_return.clone();
        let old_in_async_function = self.in_async_function;
        let old_loop_depth = self.loop_depth;
        let old_param_types = std::mem::take(&mut self.param_types);

        self.self_type = self_type;
        self.self_is_mutable = function
            .params
            .first()
            .and_then(|param| match param {
                Param::Receiver(receiver) => {
                    Some(matches!(receiver.kind, ql_ast::ReceiverKind::Mutable))
                }
                Param::Regular(_) => None,
            })
            .unwrap_or(false);
        self.current_return = Some(
            function
                .return_type
                .map(|type_id| lower_type(self.module, self.resolution, type_id))
                .unwrap_or_else(void_ty),
        );
        self.in_async_function = function.is_async;
        self.loop_depth = 0;

        if let Some(scope) = function_scope(function, self.resolution) {
            for (index, param) in function.params.iter().enumerate() {
                if let Param::Regular(param) = param {
                    let binding = ParamBinding { scope, index };
                    let ty = lower_type(self.module, self.resolution, param.ty);
                    self.param_types.insert(binding, ty);
                }
            }
        }

        if let Some(body) = function.body {
            let actual = self.check_block(body);
            let expected_return = self.current_return.clone();
            if let Some(expected) = &expected_return {
                if self.block_flow(body).guarantees_return() {
                    // Explicit `return` statements inside the body are checked as
                    // they are encountered, so a body that guarantees return does
                    // not need a synthetic tail-based mismatch.
                } else if self.module.block(body).tail.is_some() {
                    self.report_type_mismatch_expr(
                        self.module.block(body).tail,
                        expected,
                        &actual,
                        "function body",
                    );
                } else {
                    self.report_type_mismatch_block(body, expected, &actual, "function body");
                }
            }
        }

        self.self_type = old_self;
        self.self_is_mutable = old_self_is_mutable;
        self.current_return = old_return;
        self.in_async_function = old_in_async_function;
        self.loop_depth = old_loop_depth;
        self.param_types = old_param_types;
    }

    fn check_block(&mut self, block_id: BlockId) -> Ty {
        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                StmtKind::Let {
                    mutable,
                    pattern,
                    value,
                } => {
                    let value_ty = self.check_expr(*value, None);
                    self.bind_pattern(*pattern, &value_ty);
                    if *mutable {
                        self.record_mutable_pattern_bindings(*pattern);
                    }
                }
                StmtKind::Return(expr) => {
                    let expected_return = self.current_return.clone();
                    let actual = expr
                        .map(|expr_id| self.check_expr(expr_id, expected_return.as_ref()))
                        .unwrap_or_else(void_ty);
                    if let Some(expected) = &expected_return {
                        self.report_type_mismatch_stmt(stmt_id, expected, &actual, "return value");
                    }
                }
                StmtKind::Defer(expr) => {
                    self.check_expr(*expr, None);
                }
                StmtKind::Break => {
                    if self.loop_depth == 0 {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "`break` is only allowed inside loop bodies".to_string(),
                            )
                            .with_label(
                                Label::new(self.module.stmt(stmt_id).span)
                                    .with_message("`break` used here"),
                            ),
                        );
                    }
                }
                StmtKind::Continue => {
                    if self.loop_depth == 0 {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "`continue` is only allowed inside loop bodies".to_string(),
                            )
                            .with_label(
                                Label::new(self.module.stmt(stmt_id).span)
                                    .with_message("`continue` used here"),
                            ),
                        );
                    }
                }
                StmtKind::While { condition, body } => {
                    self.check_bool_condition(*condition, "while condition");
                    self.loop_depth += 1;
                    self.check_block(*body);
                    self.loop_depth -= 1;
                }
                StmtKind::Loop { body } => {
                    self.loop_depth += 1;
                    self.check_block(*body);
                    self.loop_depth -= 1;
                }
                StmtKind::For {
                    is_await,
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    if *is_await && !self.in_async_function {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "`for await` is only allowed inside `async fn`".to_string(),
                            )
                            .with_label(
                                Label::new(self.module.stmt(stmt_id).span)
                                    .with_message("`for await` used here"),
                            ),
                        );
                    }
                    self.check_expr(*iterable, None);
                    self.bind_pattern(*pattern, &Ty::Unknown);
                    self.loop_depth += 1;
                    self.check_block(*body);
                    self.loop_depth -= 1;
                }
                StmtKind::Expr { expr, .. } => {
                    self.check_expr(*expr, None);
                }
            }
        }

        let expected_return = self.current_return.clone();
        block
            .tail
            .map(|expr_id| self.check_expr(expr_id, expected_return.as_ref()))
            .unwrap_or_else(void_ty)
    }

    fn check_expr(&mut self, expr_id: ExprId, expected: Option<&Ty>) -> Ty {
        let expr = self.module.expr(expr_id);
        let ty = match &expr.kind {
            ExprKind::Name(_) => self.type_of_name(expr_id),
            ExprKind::Integer(_) => Ty::Builtin(ql_resolve::BuiltinType::Int),
            ExprKind::String { .. } => Ty::Builtin(ql_resolve::BuiltinType::String),
            ExprKind::Bool(_) => Ty::Builtin(ql_resolve::BuiltinType::Bool),
            ExprKind::NoneLiteral => Ty::Unknown,
            ExprKind::Tuple(items) => Ty::Tuple(
                items
                    .iter()
                    .map(|&item| self.check_expr(item, None))
                    .collect(),
            ),
            ExprKind::Array(items) => self.check_array_literal(expr_id, items, expected),
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => self.check_block(*block_id),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_bool_condition(*condition, "if condition");
                let then_ty = self.check_block(*then_branch);
                if let Some(else_expr) = else_branch {
                    let else_ty = self.check_expr(*else_expr, expected);
                    self.unify_branch_types(expr_id, then_ty, else_ty, "if branches")
                } else {
                    void_ty()
                }
            }
            ExprKind::Match { value, arms } => {
                let value_ty = self.check_expr(*value, None);
                self.check_match(expr_id, &value_ty, arms, expected)
            }
            ExprKind::Closure { params, body, .. } => self.check_closure(params, *body, expected),
            ExprKind::Call { callee, args } => self.check_call(expr_id, *callee, args, expected),
            ExprKind::Member { object, field, .. } => self.check_member(expr_id, *object, field),
            ExprKind::Bracket { target, items } => self.check_bracket(expr_id, *target, items),
            ExprKind::StructLiteral { .. } => self.check_struct_literal(expr_id),
            ExprKind::Binary { left, op, right } => self.check_binary(expr_id, *left, *op, *right),
            ExprKind::Unary { op, expr } => self.check_unary(expr_id, *op, *expr),
            ExprKind::Question(expr) => {
                self.check_expr(*expr, None);
                Ty::Unknown
            }
        };
        self.expr_types.insert(expr_id, ty.clone());
        ty
    }

    fn check_unary(&mut self, expr_id: ExprId, op: ql_ast::UnaryOp, operand: ExprId) -> Ty {
        match op {
            ql_ast::UnaryOp::Not => {
                let operand_ty =
                    self.check_expr(operand, Some(&Ty::Builtin(ql_resolve::BuiltinType::Bool)));
                if operand_ty.is_unknown() {
                    return Ty::Unknown;
                }
                if operand_ty.is_bool() {
                    return Ty::Builtin(ql_resolve::BuiltinType::Bool);
                }

                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "`!` requires a `Bool` operand, found `{operand_ty}`"
                    ))
                    .with_label(
                        Label::new(self.module.expr(expr_id).span).with_message("`!` used here"),
                    ),
                );
                Ty::Unknown
            }
            ql_ast::UnaryOp::Neg => self.check_expr(operand, None),
            ql_ast::UnaryOp::Await => {
                let operand_ty = self.check_expr(operand, None);
                if !self.in_async_function {
                    self.diagnostics.push(
                        Diagnostic::error("`await` is only allowed inside `async fn`".to_string())
                            .with_label(
                                Label::new(self.module.expr(expr_id).span)
                                    .with_message("`await` used here"),
                            ),
                    );
                    return Ty::Unknown;
                }
                if let Some(result_ty) = operand_ty.task_output() {
                    return result_ty.clone();
                }

                let diagnostic = if matches!(&self.module.expr(operand).kind, ExprKind::Call { .. })
                {
                    Diagnostic::error(
                        "`await` currently requires calling an `async fn`".to_string(),
                    )
                    .with_label(
                        Label::new(self.module.expr(expr_id).span)
                            .with_message("`await` used with a non-async call"),
                    )
                } else {
                    Diagnostic::error(
                        "`await` currently requires an async task handle operand".to_string(),
                    )
                    .with_label(
                        Label::new(self.module.expr(expr_id).span)
                            .with_message("`await` used with a non-task operand"),
                    )
                };
                self.diagnostics.push(diagnostic);
                Ty::Unknown
            }
            ql_ast::UnaryOp::Spawn => {
                let operand_ty = self.check_expr(operand, None);
                if !self.in_async_function {
                    self.diagnostics.push(
                        Diagnostic::error("`spawn` is only allowed inside `async fn`".to_string())
                            .with_label(
                                Label::new(self.module.expr(expr_id).span)
                                    .with_message("`spawn` used here"),
                            ),
                    );
                    return Ty::Unknown;
                }

                if let Some(result_ty) = operand_ty.task_output() {
                    Ty::TaskHandle(Box::new(result_ty.clone()))
                } else {
                    let diagnostic = if matches!(
                        &self.module.expr(operand).kind,
                        ExprKind::Call { .. }
                    ) {
                        Diagnostic::error(
                                "`spawn` currently requires calling an `async fn` or task-handle helper"
                                    .to_string(),
                            )
                            .with_label(
                                Label::new(self.module.expr(expr_id).span)
                                    .with_message("`spawn` used with a non-task call"),
                            )
                    } else {
                        Diagnostic::error(
                            "`spawn` currently requires an async task handle operand".to_string(),
                        )
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("`spawn` used with a non-task operand"),
                        )
                    };
                    self.diagnostics.push(diagnostic);
                    Ty::Unknown
                }
            }
        }
    }

    fn type_of_name(&self, expr_id: ExprId) -> Ty {
        match self.resolution.expr_resolution(expr_id) {
            Some(ValueResolution::Local(local_id)) => self
                .local_types
                .get(local_id)
                .cloned()
                .unwrap_or(Ty::Unknown),
            Some(ValueResolution::Param(binding)) => self
                .param_types
                .get(binding)
                .cloned()
                .unwrap_or(Ty::Unknown),
            Some(ValueResolution::SelfValue) => self.self_type.clone().unwrap_or(Ty::Unknown),
            Some(ValueResolution::Function(function_ref)) => {
                Ty::from_function_ref(self.module, self.resolution, *function_ref)
            }
            Some(resolution @ ValueResolution::Item(_))
            | Some(resolution @ ValueResolution::Import(_)) => self
                .item_id_for_value_resolution(resolution)
                .map(|item_id| self.value_item_ty(item_id))
                .unwrap_or(Ty::Unknown),
            None => Ty::Unknown,
        }
    }

    fn value_item_ty(&self, item_id: ItemId) -> Ty {
        // Same-file single-segment import aliases can reuse local item value semantics
        // without pretending the full module graph already exists.
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => {
                Ty::from_function(self.module, self.resolution, function)
            }
            ItemKind::Const(global) | ItemKind::Static(global) => {
                lower_type(self.module, self.resolution, global.ty)
            }
            _ => Ty::Unknown,
        }
    }

    fn check_bool_condition(&mut self, expr_id: ExprId, context: &str) {
        let ty = self.check_expr(expr_id, Some(&Ty::Builtin(ql_resolve::BuiltinType::Bool)));
        if !ty.is_unknown() && !ty.is_bool() {
            self.diagnostics.push(
                Diagnostic::error(format!("{context} must have type `Bool`, found `{ty}`"))
                    .with_label(
                        Label::new(self.module.expr(expr_id).span).with_message("condition here"),
                    ),
            );
        }
    }

    fn check_array_literal(
        &mut self,
        expr_id: ExprId,
        items: &[ExprId],
        expected: Option<&Ty>,
    ) -> Ty {
        let expected_array = match expected {
            Some(Ty::Array { element, len }) => Some((element.as_ref(), *len)),
            _ => None,
        };
        let mut element_ty = expected_array
            .map(|(element, _)| element.clone())
            .unwrap_or(Ty::Unknown);

        for &item in items {
            let item_expected = expected_array
                .map(|(element, _)| element)
                .or_else(|| (!element_ty.is_unknown()).then_some(&element_ty));
            let item_ty = self.check_expr(item, item_expected);

            if expected_array.is_none() && element_ty.is_unknown() {
                element_ty = item_ty;
                continue;
            }

            if item_ty.is_unknown() {
                continue;
            }

            let expected_element = expected_array
                .map(|(element, _)| element)
                .unwrap_or(&element_ty);
            if !expected_element.compatible_with(&item_ty) {
                self.report_type_mismatch(item, expected_element, &item_ty, "array literal item");
            }
        }

        if let Some((_, expected_len)) = expected_array
            && items.len() != expected_len
        {
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "array literal has length mismatch: expected {} item(s), found {}",
                    expected_len,
                    items.len()
                ))
                .with_label(
                    Label::new(self.module.expr(expr_id).span).with_message("array literal here"),
                ),
            );
        }

        Ty::Array {
            element: Box::new(element_ty),
            len: items.len(),
        }
    }

    fn check_bracket(&mut self, expr_id: ExprId, target: ExprId, items: &[ExprId]) -> Ty {
        let target_ty = self.check_expr(target, None);

        if items.len() != 1 {
            for &item in items {
                self.check_expr(item, None);
            }
            return Ty::Unknown;
        }

        let index_expr = items[0];

        match &target_ty {
            Ty::Array { element, .. } => {
                let index_ty =
                    self.check_expr(index_expr, Some(&Ty::Builtin(ql_resolve::BuiltinType::Int)));
                if !index_ty.is_unknown()
                    && !index_ty.compatible_with(&Ty::Builtin(ql_resolve::BuiltinType::Int))
                {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "array index must have type `Int`, found `{index_ty}`"
                        ))
                        .with_label(
                            Label::new(self.module.expr(index_expr).span)
                                .with_message("index expression here"),
                        ),
                    );
                    return Ty::Unknown;
                }

                element.as_ref().clone()
            }
            Ty::Tuple(tuple_items) => {
                let index_ty =
                    self.check_expr(index_expr, Some(&Ty::Builtin(ql_resolve::BuiltinType::Int)));
                if !index_ty.is_unknown()
                    && !index_ty.compatible_with(&Ty::Builtin(ql_resolve::BuiltinType::Int))
                {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "tuple index must have type `Int`, found `{index_ty}`"
                        ))
                        .with_label(
                            Label::new(self.module.expr(index_expr).span)
                                .with_message("index expression here"),
                        ),
                    );
                    return Ty::Unknown;
                }

                let ExprKind::Integer(raw_index) = &self.module.expr(index_expr).kind else {
                    return Ty::Unknown;
                };
                let Some(index) = ql_ast::parse_usize_literal(raw_index) else {
                    return Ty::Unknown;
                };

                if let Some(item_ty) = tuple_items.get(index) {
                    item_ty.clone()
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "tuple index `{index}` is out of bounds for tuple of length {}",
                            tuple_items.len()
                        ))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("index expression here"),
                        ),
                    );
                    Ty::Unknown
                }
            }
            _ => {
                for &item in items {
                    self.check_expr(item, None);
                }
                if self.should_report_invalid_index_target(&target_ty) {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "indexing is not supported on type `{target_ty}`; only arrays and tuples are indexable"
                        ))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("index access here"),
                        ),
                    );
                }
                Ty::Unknown
            }
        }
    }

    fn check_closure(&mut self, params: &[LocalId], body: ExprId, expected: Option<&Ty>) -> Ty {
        let param_types = match expected {
            Some(Ty::Callable {
                params: expected_params,
                ..
            }) if expected_params.len() == params.len() => expected_params.clone(),
            _ => vec![Ty::Unknown; params.len()],
        };

        for (&local_id, ty) in params.iter().zip(param_types.iter().cloned()) {
            self.local_types.insert(local_id, ty);
        }

        let expected_ret = match expected {
            Some(Ty::Callable { ret, .. }) => Some(ret.as_ref()),
            _ => None,
        };
        let old_return = self.current_return.clone();
        let old_in_async_function = self.in_async_function;
        let old_loop_depth = self.loop_depth;
        // Closures are not `async` today, so their bodies must not inherit an outer
        // function's async context or loop-control statements.
        self.current_return = expected_ret.cloned();
        self.in_async_function = false;
        self.loop_depth = 0;
        let body_ty = self.check_expr(body, expected_ret);
        self.current_return = old_return;
        self.in_async_function = old_in_async_function;
        self.loop_depth = old_loop_depth;
        let body_guarantees_return = self.expr_flow(body).guarantees_return();
        let closure_ret = match expected_ret {
            // Explicit `return` statements are checked against the callable
            // signature rather than the block tail type.
            Some(expected_ret) if body_guarantees_return => expected_ret.clone(),
            _ => body_ty.clone(),
        };
        if let Some(expected_ret) = expected_ret {
            if body_guarantees_return {
                // Explicit closure `return` statements already carry their own
                // diagnostics, so do not synthesize a second mismatch from the
                // block tail or expression result.
            } else {
                match &self.module.expr(body).kind {
                    ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                        if self.module.block(*block_id).tail.is_some() {
                            self.report_type_mismatch_expr(
                                self.module.block(*block_id).tail,
                                expected_ret,
                                &body_ty,
                                "closure body",
                            );
                        } else {
                            self.report_type_mismatch_block(
                                *block_id,
                                expected_ret,
                                &body_ty,
                                "closure body",
                            );
                        }
                    }
                    _ => self.report_type_mismatch(body, expected_ret, &body_ty, "closure body"),
                }
            }
        }

        Ty::Callable {
            params: params
                .iter()
                .map(|local_id| {
                    self.local_types
                        .get(local_id)
                        .cloned()
                        .unwrap_or(Ty::Unknown)
                })
                .collect(),
            ret: Box::new(closure_ret),
        }
    }

    fn expr_flow(&self, expr_id: ExprId) -> ControlFlowSummary {
        match &self.module.expr(expr_id).kind {
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => self.block_flow(*block_id),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let then_flow = self.block_flow(*then_branch);
                let else_flow = else_branch
                    .map(|expr_id| self.expr_flow(expr_id))
                    .unwrap_or_else(ControlFlowSummary::normal);
                let branch_flow = match self.bool_literal(*condition) {
                    Some(true) => then_flow,
                    Some(false) => else_flow,
                    None => then_flow.union(else_flow),
                };
                self.expr_flow(*condition).then(branch_flow)
            }
            ExprKind::Match { value, arms } => {
                let match_flow = if let Some(literal) = self.bool_literal(*value) {
                    self.literal_bool_match_flow(literal, arms)
                } else if self.expr_types.get(value).is_some_and(Ty::is_bool) {
                    self.bool_match_flow(arms)
                } else if let Some(Ty::Item { item_id, .. }) = self.expr_types.get(value) {
                    if matches!(self.module.item(*item_id).kind, ItemKind::Enum(_)) {
                        self.enum_match_flow(*item_id, arms)
                    } else {
                        let arms_flow = arms
                            .iter()
                            .fold(ControlFlowSummary::diverges(), |flow, arm| {
                                flow.union(self.match_arm_flow(arm))
                            });
                        if self.match_is_exhaustive(*value, arms) {
                            arms_flow
                        } else {
                            arms_flow.union(ControlFlowSummary::normal())
                        }
                    }
                } else {
                    let arms_flow = arms
                        .iter()
                        .fold(ControlFlowSummary::diverges(), |flow, arm| {
                            flow.union(self.match_arm_flow(arm))
                        });
                    if self.match_is_exhaustive(*value, arms) {
                        arms_flow
                    } else {
                        arms_flow.union(ControlFlowSummary::normal())
                    }
                };
                self.expr_flow(*value).then(match_flow)
            }
            // Nested closures are independent callable bodies, so their `return`
            // statements must not satisfy the outer function/closure body.
            ExprKind::Closure { .. } => ControlFlowSummary::normal(),
            ExprKind::Call { callee, args } => {
                let mut flow = self.expr_flow(*callee);
                for arg in args {
                    flow = flow.then(self.call_arg_flow(arg));
                }
                flow
            }
            ExprKind::Member { object, .. } => self.expr_flow(*object),
            ExprKind::Bracket { target, items } => {
                let mut flow = self.expr_flow(*target);
                for &item in items {
                    flow = flow.then(self.expr_flow(item));
                }
                flow
            }
            ExprKind::StructLiteral { fields, .. } => {
                let mut flow = ControlFlowSummary::normal();
                for field in fields {
                    flow = flow.then(self.expr_flow(field.value));
                }
                flow
            }
            ExprKind::Binary { left, op, right } => match op {
                BinaryOp::AndAnd => {
                    let left_flow = self.expr_flow(*left);
                    let right_flow = self.expr_flow(*right);
                    match self.bool_literal(*left) {
                        Some(true) => left_flow.then(right_flow),
                        Some(false) => left_flow.then(ControlFlowSummary::normal()),
                        None => left_flow.then(right_flow.union(ControlFlowSummary::normal())),
                    }
                }
                BinaryOp::OrOr => {
                    let left_flow = self.expr_flow(*left);
                    let right_flow = self.expr_flow(*right);
                    match self.bool_literal(*left) {
                        Some(true) => left_flow.then(ControlFlowSummary::normal()),
                        Some(false) => left_flow.then(right_flow),
                        None => left_flow.then(right_flow.union(ControlFlowSummary::normal())),
                    }
                }
                _ => self.expr_flow(*left).then(self.expr_flow(*right)),
            },
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => self.expr_flow(*expr),
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => ControlFlowSummary::normal(),
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                let mut flow = ControlFlowSummary::normal();
                for &item in items {
                    flow = flow.then(self.expr_flow(item));
                }
                flow
            }
        }
    }

    fn block_flow(&self, block_id: BlockId) -> ControlFlowSummary {
        let block = self.module.block(block_id);
        let mut flow = ControlFlowSummary::normal();
        for &stmt_id in &block.statements {
            flow = flow.then(self.stmt_flow(stmt_id));
        }
        if let Some(tail) = block.tail {
            flow = flow.then(self.expr_flow(tail));
        }
        flow
    }

    fn stmt_flow(&self, stmt_id: StmtId) -> ControlFlowSummary {
        match &self.module.stmt(stmt_id).kind {
            StmtKind::Return(value) => value
                .map(|expr_id| self.expr_flow(expr_id).then(ControlFlowSummary::returns()))
                .unwrap_or_else(ControlFlowSummary::returns),
            StmtKind::Let { value, .. } | StmtKind::Defer(value) => self.expr_flow(*value),
            StmtKind::While { condition, body } => {
                let condition_flow = self.expr_flow(*condition);
                let body_flow = self.block_flow(*body);
                let loop_flow = match self.bool_literal(*condition) {
                    Some(true) => ControlFlowSummary {
                        falls_through: body_flow.breaks || body_flow.continues,
                        returns: body_flow.returns,
                        ..ControlFlowSummary::default()
                    },
                    Some(false) => ControlFlowSummary::normal(),
                    None => ControlFlowSummary {
                        falls_through: true,
                        returns: body_flow.returns,
                        ..ControlFlowSummary::default()
                    },
                };
                condition_flow.then(loop_flow)
            }
            StmtKind::Loop { body } => {
                let body_flow = self.block_flow(*body);
                ControlFlowSummary {
                    falls_through: body_flow.breaks || body_flow.continues,
                    returns: body_flow.returns,
                    ..ControlFlowSummary::default()
                }
            }
            StmtKind::For { iterable, body, .. } => {
                self.expr_flow(*iterable).then(ControlFlowSummary {
                    falls_through: true,
                    returns: self.block_flow(*body).returns,
                    ..ControlFlowSummary::default()
                })
            }
            StmtKind::Expr { expr, .. } => self.expr_flow(*expr),
            StmtKind::Break => ControlFlowSummary::breaks(),
            StmtKind::Continue => ControlFlowSummary::continues(),
        }
    }

    fn call_arg_flow(&self, arg: &CallArg) -> ControlFlowSummary {
        match arg {
            CallArg::Positional(expr_id) => self.expr_flow(*expr_id),
            CallArg::Named { value, .. } => self.expr_flow(*value),
        }
    }

    fn bool_literal(&self, expr_id: ExprId) -> Option<bool> {
        let mut visited = HashSet::new();
        self.bool_literal_expr(expr_id, &mut visited)
    }

    fn bool_literal_expr(&self, expr_id: ExprId, visited: &mut HashSet<ItemId>) -> Option<bool> {
        match &self.module.expr(expr_id).kind {
            ExprKind::Bool(value) => Some(*value),
            ExprKind::Unary {
                op: ql_ast::UnaryOp::Not,
                expr,
            } => self.bool_literal_expr(*expr, visited).map(|value| !value),
            ExprKind::Binary { left, op, right } => {
                if let (Some(left), Some(right)) = (
                    self.bool_literal_expr(*left, visited),
                    self.bool_literal_expr(*right, visited),
                ) {
                    match op {
                        BinaryOp::OrOr => Some(left || right),
                        BinaryOp::AndAnd => Some(left && right),
                        BinaryOp::EqEq => Some(left == right),
                        BinaryOp::BangEq => Some(left != right),
                        _ => None,
                    }
                } else {
                    let left = self.int_literal_expr(*left, visited)?;
                    let right = self.int_literal_expr(*right, visited)?;
                    match op {
                        BinaryOp::EqEq => Some(left == right),
                        BinaryOp::BangEq => Some(left != right),
                        BinaryOp::Gt => Some(left > right),
                        BinaryOp::GtEq => Some(left >= right),
                        BinaryOp::Lt => Some(left < right),
                        BinaryOp::LtEq => Some(left <= right),
                        _ => None,
                    }
                }
            }
            ExprKind::Name(_)
            | ExprKind::Member { .. }
            | ExprKind::Bracket { .. }
            | ExprKind::Block(_)
            | ExprKind::Unsafe(_)
            | ExprKind::Question(_) => {
                let source = self.const_source_expr(expr_id, visited)?;
                if source == expr_id {
                    None
                } else {
                    self.bool_literal_expr(source, visited)
                }
            }
            _ => None,
        }
    }

    fn int_literal_expr(&self, expr_id: ExprId, visited: &mut HashSet<ItemId>) -> Option<i64> {
        match &self.module.expr(expr_id).kind {
            ExprKind::Integer(value) => ql_ast::parse_i64_literal(value),
            ExprKind::Unary {
                op: ql_ast::UnaryOp::Neg,
                expr,
            } => self
                .int_literal_expr(*expr, visited)
                .and_then(|value| value.checked_neg()),
            ExprKind::Binary { left, op, right } => {
                let left = self.int_literal_expr(*left, visited)?;
                let right = self.int_literal_expr(*right, visited)?;
                match op {
                    BinaryOp::Add => left.checked_add(right),
                    BinaryOp::Sub => left.checked_sub(right),
                    BinaryOp::Mul => left.checked_mul(right),
                    BinaryOp::Div => left.checked_div(right),
                    BinaryOp::Rem => left.checked_rem(right),
                    _ => None,
                }
            }
            ExprKind::Name(_)
            | ExprKind::Member { .. }
            | ExprKind::Bracket { .. }
            | ExprKind::Block(_)
            | ExprKind::Unsafe(_)
            | ExprKind::Question(_) => {
                let source = self.const_source_expr(expr_id, visited)?;
                if source == expr_id {
                    None
                } else {
                    self.int_literal_expr(source, visited)
                }
            }
            _ => None,
        }
    }

    fn path_pattern_const_source_expr(&self, pattern_id: PatternId) -> Option<ExprId> {
        let item_id = self
            .resolution
            .pattern_resolution(pattern_id)
            .and_then(|resolution| self.item_id_for_value_resolution(resolution))?;
        let mut visited = HashSet::new();
        self.const_source_item(item_id, &mut visited)
    }

    fn path_pattern_bool_literal(&self, pattern_id: PatternId) -> Option<bool> {
        let source = self.path_pattern_const_source_expr(pattern_id)?;
        let mut visited = HashSet::new();
        self.bool_literal_expr(source, &mut visited)
    }

    fn path_pattern_int_literal(&self, pattern_id: PatternId) -> Option<i64> {
        let source = self.path_pattern_const_source_expr(pattern_id)?;
        let mut visited = HashSet::new();
        self.int_literal_expr(source, &mut visited)
    }

    fn const_item_path_pattern_ty(&self, item_id: ItemId) -> Option<Ty> {
        let mut visited = HashSet::new();
        let source = self.const_source_item(item_id, &mut visited)?;

        let mut bool_visited = HashSet::new();
        if self.bool_literal_expr(source, &mut bool_visited).is_some() {
            return Some(Ty::Builtin(ql_resolve::BuiltinType::Bool));
        }

        let mut int_visited = HashSet::new();
        self.int_literal_expr(source, &mut int_visited)
            .map(|_| Ty::Builtin(ql_resolve::BuiltinType::Int))
    }

    fn const_source_expr(&self, expr_id: ExprId, visited: &mut HashSet<ItemId>) -> Option<ExprId> {
        match &self.module.expr(expr_id).kind {
            ExprKind::Bool(_)
            | ExprKind::Integer(_)
            | ExprKind::Unary {
                op: ql_ast::UnaryOp::Not,
                ..
            }
            | ExprKind::Unary {
                op: ql_ast::UnaryOp::Neg,
                ..
            }
            | ExprKind::Binary {
                op:
                    BinaryOp::AndAnd
                    | BinaryOp::OrOr
                    | BinaryOp::EqEq
                    | BinaryOp::BangEq
                    | BinaryOp::Gt
                    | BinaryOp::GtEq
                    | BinaryOp::Lt
                    | BinaryOp::LtEq,
                ..
            }
            | ExprKind::Binary {
                op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
                ..
            }
            | ExprKind::Tuple(_)
            | ExprKind::Array(_)
            | ExprKind::StructLiteral { .. } => Some(expr_id),
            ExprKind::Name(_) => self
                .resolution
                .expr_resolution(expr_id)
                .and_then(|resolution| self.item_id_for_value_resolution(resolution))
                .and_then(|item_id| self.const_source_item(item_id, visited)),
            ExprKind::Member { object, field, .. } => {
                let object = self.const_source_expr(*object, visited)?;
                let ExprKind::StructLiteral { fields, .. } = &self.module.expr(object).kind else {
                    return None;
                };
                let value = fields
                    .iter()
                    .find(|candidate| candidate.name == *field)?
                    .value;
                self.const_source_expr(value, visited).or(Some(value))
            }
            ExprKind::Bracket { target, items } if items.len() == 1 => {
                let index = self.int_literal_expr(items[0], visited)?;
                if index < 0 {
                    return None;
                }
                let index = index as usize;
                let target = self.const_source_expr(*target, visited)?;
                let value = match &self.module.expr(target).kind {
                    ExprKind::Tuple(items) | ExprKind::Array(items) => items.get(index).copied(),
                    _ => None,
                }?;
                self.const_source_expr(value, visited).or(Some(value))
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => self
                .module
                .block(*block_id)
                .tail
                .and_then(|tail| self.const_source_expr(tail, visited)),
            ExprKind::Question(inner) => self.const_source_expr(*inner, visited),
            _ => None,
        }
    }

    fn const_source_item(&self, item_id: ItemId, visited: &mut HashSet<ItemId>) -> Option<ExprId> {
        if !visited.insert(item_id) {
            return None;
        }

        let result = match &self.module.item(item_id).kind {
            ItemKind::Const(global) | ItemKind::Static(global) => {
                self.const_source_expr(global.value, visited)
            }
            _ => None,
        };

        visited.remove(&item_id);
        result
    }

    fn ordered_match_flow<F>(&self, arms: &[MatchArm], mut pattern_matches: F) -> ControlFlowSummary
    where
        F: FnMut(&Self, PatternId) -> bool,
    {
        let mut flow = ControlFlowSummary::diverges();
        let mut pending = true;

        for arm in arms {
            if !pending {
                break;
            }
            if !pattern_matches(self, arm.pattern) {
                continue;
            }

            let body_flow = self.expr_flow(arm.body);
            match arm.guard {
                None => {
                    flow = flow.union(body_flow);
                    pending = false;
                }
                Some(guard) => {
                    let guard_flow = self.expr_flow(guard);
                    flow = flow.union(ControlFlowSummary {
                        falls_through: false,
                        returns: guard_flow.returns,
                        breaks: guard_flow.breaks,
                        continues: guard_flow.continues,
                    });
                    if !guard_flow.falls_through {
                        pending = false;
                        continue;
                    }

                    match self.arm_guard_literal(arm) {
                        Some(true) => {
                            flow = flow.union(body_flow);
                            pending = false;
                        }
                        Some(false) => {}
                        None => {
                            flow = flow.union(body_flow);
                        }
                    }
                }
            }
        }

        if pending {
            flow = flow.union(ControlFlowSummary::normal());
        }

        flow
    }

    fn bool_match_flow(&self, arms: &[MatchArm]) -> ControlFlowSummary {
        self.literal_bool_match_flow(true, arms)
            .union(self.literal_bool_match_flow(false, arms))
    }

    fn literal_bool_match_flow(&self, value: bool, arms: &[MatchArm]) -> ControlFlowSummary {
        self.ordered_match_flow(arms, |this, pattern_id| {
            this.pattern_matches_bool_literal(pattern_id, value)
        })
    }

    fn enum_match_flow(&self, enum_item_id: ItemId, arms: &[MatchArm]) -> ControlFlowSummary {
        let item = self.module.item(enum_item_id);
        let ItemKind::Enum(enum_decl) = &item.kind else {
            return ControlFlowSummary::normal();
        };

        enum_decl
            .variants
            .iter()
            .fold(ControlFlowSummary::diverges(), |flow, variant| {
                flow.union(self.ordered_match_flow(arms, |this, pattern_id| {
                    this.pattern_matches_enum_variant(
                        enum_item_id,
                        pattern_id,
                        variant.name.as_str(),
                    )
                }))
            })
    }

    fn pattern_matches_bool_literal(&self, pattern_id: PatternId, value: bool) -> bool {
        match &self.module.pattern(pattern_id).kind {
            PatternKind::Bool(pattern_value) => *pattern_value == value,
            PatternKind::Path(_) => self.path_pattern_bool_literal(pattern_id) == Some(value),
            PatternKind::Binding(_) | PatternKind::Wildcard => true,
            _ => false,
        }
    }

    fn pattern_matches_enum_variant(
        &self,
        enum_item_id: ItemId,
        pattern_id: PatternId,
        variant_name: &str,
    ) -> bool {
        match &self.module.pattern(pattern_id).kind {
            PatternKind::Binding(_) | PatternKind::Wildcard => true,
            _ => self
                .enum_variant_name_for_pattern(enum_item_id, pattern_id)
                .is_some_and(|name| name == variant_name),
        }
    }

    fn arm_guard_literal(&self, arm: &MatchArm) -> Option<bool> {
        arm.guard.and_then(|guard| self.bool_literal(guard))
    }

    fn arm_counts_for_exhaustiveness(&self, arm: &MatchArm) -> bool {
        match arm.guard {
            None => true,
            Some(_) => matches!(self.arm_guard_literal(arm), Some(true)),
        }
    }

    fn match_arm_flow(&self, arm: &MatchArm) -> ControlFlowSummary {
        let body_flow = self.expr_flow(arm.body);
        match arm.guard {
            Some(guard) => {
                let guard_flow = self.expr_flow(guard);
                match self.arm_guard_literal(arm) {
                    Some(true) => guard_flow.then(body_flow),
                    Some(false) => guard_flow.then(ControlFlowSummary::normal()),
                    None => guard_flow.then(body_flow.union(ControlFlowSummary::normal())),
                }
            }
            None => body_flow,
        }
    }

    fn match_has_catch_all_arm(&self, arms: &[MatchArm]) -> bool {
        arms.iter().any(|arm| {
            matches!(
                self.module.pattern(arm.pattern).kind,
                PatternKind::Binding(_) | PatternKind::Wildcard
            ) && self.arm_counts_for_exhaustiveness(arm)
        })
    }

    fn match_is_exhaustive(&self, value: ExprId, arms: &[MatchArm]) -> bool {
        if self.match_has_catch_all_arm(arms) {
            return true;
        }

        let Some(value_ty) = self.expr_types.get(&value) else {
            return false;
        };

        if value_ty.is_bool() {
            return self.match_covers_all_bool_patterns(arms);
        }

        let Ty::Item { item_id, .. } = value_ty else {
            return false;
        };

        self.match_covers_all_enum_variants(*item_id, arms)
    }

    fn match_covers_all_bool_patterns(&self, arms: &[MatchArm]) -> bool {
        let mut saw_true = false;
        let mut saw_false = false;

        for arm in arms {
            if !self.arm_counts_for_exhaustiveness(arm) {
                continue;
            }
            if self.pattern_matches_bool_literal(arm.pattern, true) {
                saw_true = true;
            }
            if self.pattern_matches_bool_literal(arm.pattern, false) {
                saw_false = true;
            }
        }

        saw_true && saw_false
    }

    fn match_covers_all_enum_variants(&self, enum_item_id: ItemId, arms: &[MatchArm]) -> bool {
        let item = self.module.item(enum_item_id);
        let ItemKind::Enum(enum_decl) = &item.kind else {
            return false;
        };

        let mut seen_variants = HashSet::new();
        for arm in arms {
            if !self.arm_counts_for_exhaustiveness(arm) {
                continue;
            }
            if let Some(variant_name) =
                self.enum_variant_name_for_pattern(enum_item_id, arm.pattern)
            {
                seen_variants.insert(variant_name.to_owned());
            }
        }

        enum_decl
            .variants
            .iter()
            .all(|variant| seen_variants.contains(&variant.name))
    }

    fn enum_variant_name_for_pattern(
        &self,
        enum_item_id: ItemId,
        pattern_id: PatternId,
    ) -> Option<&str> {
        let pattern = self.module.pattern(pattern_id);
        let path = match &pattern.kind {
            PatternKind::Path(path)
            | PatternKind::TupleStruct { path, .. }
            | PatternKind::Struct { path, .. } => path,
            _ => return None,
        };

        let resolved_item_id = self
            .resolution
            .pattern_resolution(pattern_id)
            .and_then(|resolution| self.item_id_for_value_resolution(resolution))?;
        if resolved_item_id != enum_item_id {
            return None;
        }

        self.enum_variant_for_item_path(enum_item_id, path)
            .ok()
            .flatten()
            .map(|variant| variant.name.as_str())
    }

    fn check_call(
        &mut self,
        expr_id: ExprId,
        callee: ExprId,
        args: &[CallArg],
        _expected: Option<&Ty>,
    ) -> Ty {
        let callee_ty = self.check_expr(callee, None);
        let signature = self.call_signature(callee, &callee_ty);
        let Some(signature) = signature else {
            if !callee_ty.is_unknown() {
                self.diagnostics.push(
                    Diagnostic::error(format!("cannot call value of type `{callee_ty}`"))
                        .with_label(
                            Label::new(self.module.expr(callee).span).with_message("callee here"),
                        ),
                );
            }
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => {
                        self.check_expr(*expr, None);
                    }
                    CallArg::Named { value, .. } => {
                        self.check_expr(*value, None);
                    }
                }
            }
            return Ty::Unknown;
        };

        self.check_call_args(expr_id, &signature, args);
        if signature.is_async {
            Ty::TaskHandle(Box::new(signature.ret))
        } else {
            signature.ret
        }
    }

    fn check_member(&mut self, expr_id: ExprId, object: ExprId, field: &str) -> Ty {
        let object_ty = self.check_expr(object, None);
        let Ty::Item { .. } = &object_ty else {
            if self.should_report_invalid_member_receiver(&object_ty) {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "member access is not supported on type `{object_ty}`"
                    ))
                    .with_label(
                        Label::new(self.module.expr(expr_id).span)
                            .with_message("member access here"),
                    ),
                );
            }
            return Ty::Unknown;
        };

        match self.select_member_target(&object_ty, field) {
            SelectedMember::Field { target, ty } => {
                self.member_targets
                    .insert(expr_id, MemberTarget::Field(target));
                ty
            }
            SelectedMember::Method(target) => {
                self.member_targets
                    .insert(expr_id, MemberTarget::Method(target));
                Ty::Unknown
            }
            SelectedMember::AmbiguousMethod => {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "ambiguous method `{field}` on type `{object_ty}`; multiple matching methods found"
                    ))
                    .with_label(
                        Label::new(self.module.expr(expr_id).span)
                            .with_message("member access here"),
                    ),
                );
                Ty::Unknown
            }
            SelectedMember::Missing => {
                self.diagnostics.push(
                    Diagnostic::error(format!("unknown member `{field}` on type `{object_ty}`"))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("member access here"),
                        ),
                );
                Ty::Unknown
            }
        }
    }

    fn should_report_invalid_member_receiver(&self, ty: &Ty) -> bool {
        matches!(
            ty,
            Ty::Builtin(_)
                | Ty::Array { .. }
                | Ty::Pointer { .. }
                | Ty::Tuple(_)
                | Ty::Callable { .. }
        )
    }

    fn should_report_invalid_index_target(&self, ty: &Ty) -> bool {
        matches!(
            ty,
            Ty::Builtin(_) | Ty::Pointer { .. } | Ty::Callable { .. } | Ty::Item { .. }
        )
    }

    fn select_member_target(&self, object_ty: &Ty, field: &str) -> SelectedMember {
        let Ty::Item { item_id, .. } = object_ty else {
            return SelectedMember::Missing;
        };

        match self.select_method_target(object_ty, field, MethodSource::Impl) {
            MethodSelection::Unique(target) => return SelectedMember::Method(target),
            MethodSelection::Ambiguous => return SelectedMember::AmbiguousMethod,
            MethodSelection::None => {}
        }

        match self.select_method_target(object_ty, field, MethodSource::Extend) {
            MethodSelection::Unique(target) => return SelectedMember::Method(target),
            MethodSelection::Ambiguous => return SelectedMember::AmbiguousMethod,
            MethodSelection::None => {}
        }

        match &self.module.item(*item_id).kind {
            ItemKind::Struct(struct_decl) => struct_decl
                .fields
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.name == field)
                .map(|(field_index, candidate)| SelectedMember::Field {
                    target: FieldTarget {
                        item_id: *item_id,
                        field_index,
                    },
                    ty: lower_type(self.module, self.resolution, candidate.ty),
                })
                .unwrap_or(SelectedMember::Missing),
            _ => SelectedMember::Missing,
        }
    }

    fn select_method_target(
        &self,
        object_ty: &Ty,
        field: &str,
        source: MethodSource,
    ) -> MethodSelection {
        let mut matched_method = None;
        let mut ambiguous_method = false;

        for &candidate_item_id in &self.module.items {
            match &self.module.item(candidate_item_id).kind {
                ItemKind::Impl(impl_block) if source == MethodSource::Impl => {
                    let target_ty = lower_type(self.module, self.resolution, impl_block.target);
                    if object_ty.compatible_with(&target_ty) {
                        for (method_index, method) in impl_block.methods.iter().enumerate() {
                            if method.name == field {
                                if matched_method.is_some() {
                                    ambiguous_method = true;
                                } else {
                                    matched_method = Some(MethodTarget {
                                        item_id: candidate_item_id,
                                        method_index,
                                    });
                                }
                            }
                        }
                    }
                }
                ItemKind::Extend(extend_block) if source == MethodSource::Extend => {
                    let target_ty = lower_type(self.module, self.resolution, extend_block.target);
                    if object_ty.compatible_with(&target_ty) {
                        for (method_index, method) in extend_block.methods.iter().enumerate() {
                            if method.name == field {
                                if matched_method.is_some() {
                                    ambiguous_method = true;
                                } else {
                                    matched_method = Some(MethodTarget {
                                        item_id: candidate_item_id,
                                        method_index,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if ambiguous_method {
            MethodSelection::Ambiguous
        } else if let Some(target) = matched_method {
            MethodSelection::Unique(target)
        } else {
            MethodSelection::None
        }
    }

    fn call_signature(&self, callee: ExprId, callee_ty: &Ty) -> Option<Signature> {
        if matches!(self.module.expr(callee).kind, ExprKind::Name(_))
            && let Some(resolution) = self.resolution.expr_resolution(callee)
        {
            match resolution {
                ValueResolution::Function(function_ref) => {
                    let function = self.module.function(*function_ref);
                    return Some(Signature::from_function(
                        self.module,
                        self.resolution,
                        function,
                    ));
                }
                ValueResolution::Item(_) | ValueResolution::Import(_) => {
                    if let Some(item_id) = self.item_id_for_value_resolution(resolution)
                        && let Some(signature) = self.value_item_signature(item_id)
                    {
                        return Some(signature);
                    }
                }
                ValueResolution::Local(_)
                | ValueResolution::Param(_)
                | ValueResolution::SelfValue => {}
            }
        }
        if let Some(MemberTarget::Method(target)) = self.member_targets.get(&callee).copied() {
            let function = self.method(target);
            return Some(Signature::from_function(
                self.module,
                self.resolution,
                function,
            ));
        }

        match callee_ty {
            Ty::Callable { params, ret } => Some(Signature {
                is_async: false,
                params: params
                    .iter()
                    .cloned()
                    .map(|ty| SignatureParam { name: None, ty })
                    .collect(),
                ret: ret.as_ref().clone(),
            }),
            _ => None,
        }
    }

    fn value_item_signature(&self, item_id: ItemId) -> Option<Signature> {
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => Some(Signature::from_function(
                self.module,
                self.resolution,
                function,
            )),
            _ => None,
        }
    }

    fn method(&self, target: MethodTarget) -> &Function {
        match &self.module.item(target.item_id).kind {
            ItemKind::Trait(trait_decl) => &trait_decl.methods[target.method_index],
            ItemKind::Impl(impl_block) => &impl_block.methods[target.method_index],
            ItemKind::Extend(extend_block) => &extend_block.methods[target.method_index],
            other => panic!("expected method-bearing item, got {other:?}"),
        }
    }

    fn check_call_args(&mut self, expr_id: ExprId, signature: &Signature, args: &[CallArg]) {
        let named = args.iter().any(|arg| matches!(arg, CallArg::Named { .. }));
        if !named {
            if args.len() != signature.params.len() {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "expected {} argument(s), found {}",
                        signature.params.len(),
                        args.len()
                    ))
                    .with_label(
                        Label::new(self.module.expr(expr_id).span).with_message("call here"),
                    ),
                );
            }

            for (arg, param) in args.iter().zip(&signature.params) {
                let CallArg::Positional(arg_expr) = arg else {
                    continue;
                };
                let actual = self.check_expr(*arg_expr, Some(&param.ty));
                self.report_type_mismatch(*arg_expr, &param.ty, &actual, "call argument");
            }
            return;
        }

        let mut seen = HashSet::new();
        for arg in args {
            let CallArg::Named {
                name,
                name_span,
                value,
            } = arg
            else {
                let expr = match arg {
                    CallArg::Positional(expr) => *expr,
                    CallArg::Named { value, .. } => *value,
                };
                self.check_expr(expr, None);
                continue;
            };

            let Some(param) = signature
                .params
                .iter()
                .find(|param| param.name.as_deref() == Some(name.as_str()))
            else {
                self.diagnostics.push(
                    Diagnostic::error(format!("unknown named argument `{name}`"))
                        .with_label(Label::new(*name_span).with_message("argument label here")),
                );
                self.check_expr(*value, None);
                continue;
            };

            seen.insert(name.clone());
            let actual = self.check_expr(*value, Some(&param.ty));
            self.report_type_mismatch(*value, &param.ty, &actual, "named call argument");
        }

        for param in &signature.params {
            if let Some(name) = &param.name
                && !seen.contains(name)
            {
                self.diagnostics.push(
                    Diagnostic::error(format!("missing argument `{name}`")).with_label(
                        Label::new(self.module.expr(expr_id).span).with_message("call here"),
                    ),
                );
            }
        }
    }

    fn item_id_for_value_resolution(&self, resolution: &ValueResolution) -> Option<ItemId> {
        match resolution {
            ValueResolution::Item(item_id) => Some(*item_id),
            ValueResolution::Import(import_binding) => {
                local_item_for_import_binding(self.module, import_binding)
            }
            _ => None,
        }
    }

    fn item_id_for_type_resolution(&self, resolution: &TypeResolution) -> Option<ItemId> {
        match resolution {
            TypeResolution::Item(item_id) => Some(*item_id),
            TypeResolution::Import(import_binding) => {
                local_item_for_import_binding(self.module, import_binding)
            }
            _ => None,
        }
    }

    fn check_struct_literal(&mut self, expr_id: ExprId) -> Ty {
        let expr = self.module.expr(expr_id);
        let ExprKind::StructLiteral { path, fields } = &expr.kind else {
            return Ty::Unknown;
        };
        let item_id = self
            .resolution
            .struct_literal_resolution(expr_id)
            .and_then(|resolution| self.item_id_for_type_resolution(resolution))
            .filter(|&item_id| self.field_infos_for_item_path(item_id, path).is_some());

        let Some(item_id) = item_id else {
            if let Some(message) = self.invalid_struct_literal_root_message(expr_id, path) {
                self.diagnostics.push(
                    Diagnostic::error(message)
                        .with_label(Label::new(expr.span).with_message("struct literal here")),
                );
            }
            for field in fields {
                self.check_expr(field.value, None);
            }
            return Ty::Unknown;
        };
        let fields_info = self
            .field_infos_for_item_path(item_id, path)
            .expect("supported struct literals should expose field information");
        let root_ty = Ty::Item {
            item_id,
            name: item_display_name(self.module, item_id),
            args: Vec::new(),
        };

        let mut seen = HashSet::new();
        for field in fields {
            let Some(info) = fields_info.iter().find(|info| info.name == field.name) else {
                self.diagnostics.push(
                    Diagnostic::error(format!("unknown field `{}` in struct literal", field.name))
                        .with_label(Label::new(field.name_span).with_message("field here")),
                );
                self.check_expr(field.value, None);
                continue;
            };

            seen.insert(field.name.clone());
            let actual = self.check_expr(field.value, Some(&info.ty));
            self.report_type_mismatch(field.value, &info.ty, &actual, "struct literal field");
        }

        for info in &fields_info {
            if !info.has_default && !seen.contains(&info.name) {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "missing required field `{}` in struct literal",
                        info.name
                    ))
                    .with_label(Label::new(expr.span).with_message("struct literal here")),
                );
            }
        }

        root_ty
    }

    fn invalid_struct_literal_root_message(
        &self,
        expr_id: ExprId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        let resolution = self.resolution.struct_literal_resolution(expr_id)?;
        match resolution {
            TypeResolution::Builtin(_) | TypeResolution::Generic(_) if path.segments.len() == 1 => {
                Some(format!(
                    "struct literal syntax is not supported for `{path_text}`"
                ))
            }
            TypeResolution::Builtin(_) | TypeResolution::Generic(_) => None,
            TypeResolution::Import(import_binding) => {
                local_item_for_import_binding(self.module, import_binding).and_then(|item_id| {
                    self.invalid_struct_literal_item_path_message(item_id, path)
                })
            }
            TypeResolution::Item(item_id) => {
                self.invalid_struct_literal_item_path_message(*item_id, path)
            }
        }
    }

    fn enum_variant_for_item_path(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Result<Option<&EnumVariant>, String> {
        let item = self.module.item(item_id);
        let ItemKind::Enum(enum_decl) = &item.kind else {
            return Ok(None);
        };
        if path.segments.len() != 2 {
            return Ok(None);
        }
        let Some(variant_name) = path.segments.last() else {
            return Ok(None);
        };
        enum_decl
            .variants
            .iter()
            .find(|variant| &variant.name == variant_name)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "unknown variant `{variant_name}` in enum `{}`",
                    item_display_name(self.module, item_id)
                )
            })
    }

    fn invalid_struct_literal_item_path_message(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        match &self.module.item(item_id).kind {
            ItemKind::Struct(_) if path.segments.len() == 1 => None,
            ItemKind::Struct(_) => None,
            ItemKind::Enum(_) if path.segments.len() == 2 => {
                match self.enum_variant_for_item_path(item_id, path) {
                    Ok(Some(variant)) => match &variant.fields {
                        VariantFields::Struct(_) => None,
                        VariantFields::Tuple(_) | VariantFields::Unit => Some(format!(
                            "struct literal syntax is not supported for `{path_text}`"
                        )),
                    },
                    Err(message) => Some(message),
                    Ok(None) => None,
                }
            }
            ItemKind::Enum(_) if path.segments.len() == 1 => Some(format!(
                "struct literal syntax is not supported for `{path_text}`"
            )),
            ItemKind::Enum(_) => None,
            ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_)
                if path.segments.len() == 1 =>
            {
                Some(format!(
                    "struct literal syntax is not supported for `{path_text}`"
                ))
            }
            ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => None,
        }
    }

    fn check_binary(&mut self, expr_id: ExprId, left: ExprId, op: BinaryOp, right: ExprId) -> Ty {
        let left_ty = self.check_expr(left, None);

        match op {
            BinaryOp::Assign => {
                let assignment_policy = self.check_assignment_target(left);
                let right_ty = match assignment_policy {
                    AssignmentTargetPolicy::EnforceValueType => {
                        self.check_expr(right, Some(&left_ty))
                    }
                    AssignmentTargetPolicy::SkipValueType => self.check_expr(right, None),
                };
                if assignment_policy == AssignmentTargetPolicy::EnforceValueType {
                    self.report_type_mismatch(right, &left_ty, &right_ty, "assignment");
                }
                Ty::Unknown
            }
            BinaryOp::OrOr | BinaryOp::AndAnd => {
                let right_ty =
                    self.check_expr(right, Some(&Ty::Builtin(ql_resolve::BuiltinType::Bool)));
                if (left_ty.is_bool() || left_ty.is_unknown())
                    && (right_ty.is_bool() || right_ty.is_unknown())
                {
                    Ty::Builtin(ql_resolve::BuiltinType::Bool)
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "logical operator `{}` expects `Bool` operands, found `{}` and `{}`",
                            op_text(op),
                            left_ty,
                            right_ty
                        ))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("expression here"),
                        ),
                    );
                    Ty::Unknown
                }
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let right_ty = self.check_expr(right, None);
                if let Some(result_ty) = self.compatible_numeric_result_ty(&left_ty, &right_ty) {
                    return result_ty;
                }
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "binary operator `{}` expects numeric operands, found `{}` and `{}`",
                        op_text(op),
                        left_ty,
                        right_ty
                    ))
                    .with_label(
                        Label::new(self.module.expr(expr_id).span).with_message("expression here"),
                    ),
                );
                Ty::Unknown
            }
            BinaryOp::EqEq | BinaryOp::BangEq => {
                let right_ty = self.check_expr(right, None);
                if left_ty.compatible_with(&right_ty) {
                    Ty::Builtin(ql_resolve::BuiltinType::Bool)
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "equality operator `{}` expects compatible operands, found `{}` and `{}`",
                            op_text(op),
                            left_ty,
                            right_ty
                        ))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("expression here"),
                        ),
                    );
                    Ty::Unknown
                }
            }
            BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq => {
                let right_ty = self.check_expr(right, None);
                if self.has_compatible_numeric_operands(&left_ty, &right_ty) {
                    Ty::Builtin(ql_resolve::BuiltinType::Bool)
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "comparison operator `{}` expects compatible numeric operands, found `{}` and `{}`",
                            op_text(op),
                            left_ty,
                            right_ty
                        ))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("expression here"),
                        ),
                    );
                    Ty::Unknown
                }
            }
        }
    }

    fn check_assignment_target(&mut self, expr_id: ExprId) -> AssignmentTargetPolicy {
        let anchor_span = self.module.expr(expr_id).span;
        self.check_assignment_target_with_anchor(expr_id, anchor_span)
    }

    fn check_assignment_target_with_anchor(
        &mut self,
        expr_id: ExprId,
        anchor_span: Span,
    ) -> AssignmentTargetPolicy {
        let expr = self.module.expr(expr_id);
        let unsupported_target = |checker: &mut Self, message: String| {
            checker.diagnostics.push(
                Diagnostic::error(message)
                    .with_label(Label::new(anchor_span).with_message("assignment target here")),
            );
            AssignmentTargetPolicy::SkipValueType
        };

        match &expr.kind {
            ExprKind::Name(name) => match self.resolution.expr_resolution(expr_id) {
                Some(ValueResolution::Local(local_id)) if self.mutable_locals.contains(local_id) => {
                    AssignmentTargetPolicy::EnforceValueType
                }
                Some(ValueResolution::Local(_)) => {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "cannot assign to immutable local `{name}`; declare it with `var`"
                        ))
                        .with_label(Label::new(anchor_span).with_message("assignment target here")),
                    );
                    AssignmentTargetPolicy::EnforceValueType
                }
                Some(ValueResolution::Param(_)) => {
                    self.diagnostics.push(
                        Diagnostic::error(format!("cannot assign to immutable parameter `{name}`"))
                            .with_label(
                                Label::new(anchor_span).with_message("assignment target here"),
                            ),
                    );
                    AssignmentTargetPolicy::EnforceValueType
                }
                Some(ValueResolution::SelfValue) if self.self_is_mutable => {
                    AssignmentTargetPolicy::EnforceValueType
                }
                Some(ValueResolution::SelfValue) => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "cannot assign to immutable receiver `self`; use `var self`",
                        )
                        .with_label(Label::new(anchor_span).with_message("assignment target here")),
                    );
                    AssignmentTargetPolicy::EnforceValueType
                }
                Some(ValueResolution::Function(_)) => {
                    unsupported_target(self, format!("cannot assign to function `{name}`"))
                }
                Some(ValueResolution::Item(item_id)) => match &self.module.item(*item_id).kind {
                    ItemKind::Const(_) => {
                        unsupported_target(self, format!("cannot assign to constant `{name}`"))
                    }
                    ItemKind::Static(_) => {
                        unsupported_target(self, format!("cannot assign to static `{name}`"))
                    }
                    _ => unsupported_target(
                        self,
                        format!(
                            "cannot assign to item `{}`",
                            item_display_name(self.module, *item_id)
                        ),
                    ),
                },
                Some(ValueResolution::Import(_)) => {
                    unsupported_target(self, format!("cannot assign to imported binding `{name}`"))
                }
                None => AssignmentTargetPolicy::SkipValueType,
            },
            ExprKind::Member { object, .. } => {
                if self
                    .expr_types
                    .get(&expr_id)
                    .is_none_or(|ty| ty.is_unknown())
                {
                    AssignmentTargetPolicy::SkipValueType
                } else {
                    self.check_assignment_target_with_anchor(*object, anchor_span)
                }
            }
            ExprKind::Bracket { target, items } => {
                let Some(target_ty) = self.expr_types.get(target) else {
                    return AssignmentTargetPolicy::SkipValueType;
                };
                if target_ty.is_unknown() {
                    return AssignmentTargetPolicy::SkipValueType;
                }

                match target_ty {
                    Ty::Tuple(_) if items.len() == 1 => {
                        if matches!(self.module.expr(items[0]).kind, ExprKind::Integer(_)) {
                            self.check_assignment_target_with_anchor(*target, anchor_span)
                        } else {
                            unsupported_target(
                                self,
                                "assignment through tuple indexing currently requires an integer literal index"
                                    .to_string(),
                            )
                        }
                    }
                    Ty::Array { .. } if items.len() == 1 => {
                        self.check_assignment_target_with_anchor(*target, anchor_span)
                    }
                    _ => unsupported_target(
                        self,
                        "assignment through indexing is not supported yet; only tuple literal-index projections and array projections can be assigned"
                            .to_string(),
                    ),
                }
            }
            _ => unsupported_target(
                self,
                "this assignment target is not supported yet; only bare mutable bindings, member projections, tuple literal-index projections, and array projections can be assigned"
                    .to_string(),
            ),
        }
    }

    fn compatible_numeric_result_ty(&self, left_ty: &Ty, right_ty: &Ty) -> Option<Ty> {
        if left_ty.is_unknown() && right_ty.is_unknown() {
            return Some(Ty::Unknown);
        }
        if left_ty.is_numeric() && right_ty.is_numeric() && left_ty.compatible_with(right_ty) {
            return Some(if left_ty.is_unknown() {
                right_ty.clone()
            } else {
                left_ty.clone()
            });
        }
        if left_ty.is_unknown() && right_ty.is_numeric() {
            return Some(right_ty.clone());
        }
        if right_ty.is_unknown() && left_ty.is_numeric() {
            return Some(left_ty.clone());
        }
        None
    }

    fn has_compatible_numeric_operands(&self, left_ty: &Ty, right_ty: &Ty) -> bool {
        if left_ty.is_unknown() && right_ty.is_unknown() {
            return true;
        }
        if left_ty.is_unknown() && right_ty.is_numeric() {
            return true;
        }
        if right_ty.is_unknown() && left_ty.is_numeric() {
            return true;
        }
        left_ty.is_numeric() && right_ty.is_numeric() && left_ty.compatible_with(right_ty)
    }

    fn check_match(
        &mut self,
        expr_id: ExprId,
        value_ty: &Ty,
        arms: &[MatchArm],
        expected: Option<&Ty>,
    ) -> Ty {
        let mut result_ty: Option<Ty> = None;
        for arm in arms {
            self.bind_pattern(arm.pattern, value_ty);
            if let Some(guard) = arm.guard {
                self.check_bool_condition(guard, "match guard");
            }
            let arm_ty = self.check_expr(arm.body, expected.or(result_ty.as_ref()));
            result_ty = Some(match result_ty {
                Some(previous) => self.unify_branch_types(expr_id, previous, arm_ty, "match arms"),
                None => arm_ty,
            });
        }
        result_ty.unwrap_or(Ty::Unknown)
    }

    fn bind_pattern(&mut self, pattern_id: PatternId, expected: &Ty) {
        self.pattern_types.insert(pattern_id, expected.clone());
        let pattern = self.module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(local_id) => {
                self.local_types.insert(*local_id, expected.clone());
            }
            PatternKind::Tuple(items) => {
                if let Ty::Tuple(expected_items) = expected {
                    if expected_items.len() != items.len() {
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "tuple pattern expects {} item(s), found {}",
                                items.len(),
                                expected_items.len()
                            ))
                            .with_label(Label::new(pattern.span).with_message("pattern here")),
                        );
                    }
                    for (&item, expected_item) in items.iter().zip(expected_items) {
                        self.bind_pattern(item, expected_item);
                    }
                    for &item in items.iter().skip(expected_items.len()) {
                        self.bind_pattern(item, &Ty::Unknown);
                    }
                } else {
                    if !expected.is_unknown() {
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "tuple pattern requires a tuple value, found `{expected}`"
                            ))
                            .with_label(Label::new(pattern.span).with_message("pattern here")),
                        );
                    }
                    for &item in items {
                        self.bind_pattern(item, &Ty::Unknown);
                    }
                }
            }
            PatternKind::TupleStruct { items, .. } => {
                let invalid_root_message =
                    self.invalid_tuple_struct_pattern_root_message(pattern_id);
                if invalid_root_message.is_none() {
                    self.check_pattern_root(pattern_id, expected, "tuple-struct pattern");
                }
                let expected_items = self.tuple_struct_pattern_items(pattern_id);
                if let Some(expected_items) = expected_items {
                    if expected_items.len() != items.len() {
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "tuple-struct pattern expects {} item(s), found {}",
                                expected_items.len(),
                                items.len()
                            ))
                            .with_label(Label::new(pattern.span).with_message("pattern here")),
                        );
                    }
                    for (&item, expected_item) in items.iter().zip(expected_items.iter()) {
                        self.bind_pattern(item, expected_item);
                    }
                    for &item in items.iter().skip(expected_items.len()) {
                        self.bind_pattern(item, &Ty::Unknown);
                    }
                } else {
                    if let Some(message) = invalid_root_message {
                        self.diagnostics.push(
                            Diagnostic::error(message)
                                .with_label(Label::new(pattern.span).with_message("pattern here")),
                        );
                    }
                    for &item in items {
                        self.bind_pattern(item, &Ty::Unknown);
                    }
                }
            }
            PatternKind::Struct { fields, .. } => {
                let invalid_root_message = self.invalid_struct_pattern_root_message(pattern_id);
                if invalid_root_message.is_none() {
                    self.check_pattern_root(pattern_id, expected, "struct pattern");
                }
                let field_types = self.struct_pattern_fields(pattern_id);
                if field_types.is_none()
                    && let Some(message) = invalid_root_message
                {
                    self.diagnostics.push(
                        Diagnostic::error(message)
                            .with_label(Label::new(pattern.span).with_message("pattern here")),
                    );
                }
                for field in fields {
                    let field_ty = if let Some(field_types) = field_types.as_ref() {
                        if let Some(info) = field_types.iter().find(|info| info.name == field.name)
                        {
                            info.ty.clone()
                        } else {
                            self.diagnostics.push(
                                Diagnostic::error(format!(
                                    "unknown field `{}` in struct pattern",
                                    field.name
                                ))
                                .with_label(Label::new(field.name_span).with_message("field here")),
                            );
                            Ty::Unknown
                        }
                    } else {
                        Ty::Unknown
                    };
                    self.bind_pattern(field.pattern, &field_ty);
                }
            }
            PatternKind::Path(_) => {
                if self.path_pattern_bool_literal(pattern_id).is_some() {
                    self.check_literal_pattern(
                        pattern_id,
                        expected,
                        &Ty::Builtin(ql_resolve::BuiltinType::Bool),
                        "path pattern",
                    );
                } else if self.path_pattern_int_literal(pattern_id).is_some() {
                    self.check_literal_pattern(
                        pattern_id,
                        expected,
                        &Ty::Builtin(ql_resolve::BuiltinType::Int),
                        "path pattern",
                    );
                } else if let Some(message) = self.invalid_path_pattern_root_message(pattern_id) {
                    self.diagnostics.push(
                        Diagnostic::error(message)
                            .with_label(Label::new(pattern.span).with_message("pattern here")),
                    );
                } else {
                    self.check_pattern_root(pattern_id, expected, "path pattern");
                }
            }
            PatternKind::Integer(_) => {
                self.check_literal_pattern(
                    pattern_id,
                    expected,
                    &Ty::Builtin(ql_resolve::BuiltinType::Int),
                    "integer pattern",
                );
            }
            PatternKind::String(_) => {
                self.check_literal_pattern(
                    pattern_id,
                    expected,
                    &Ty::Builtin(ql_resolve::BuiltinType::String),
                    "string pattern",
                );
            }
            PatternKind::Bool(_) => {
                self.check_literal_pattern(
                    pattern_id,
                    expected,
                    &Ty::Builtin(ql_resolve::BuiltinType::Bool),
                    "bool pattern",
                );
            }
            PatternKind::NoneLiteral | PatternKind::Wildcard => {}
        }
    }

    fn record_mutable_pattern_bindings(&mut self, pattern_id: PatternId) {
        let pattern = self.module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(local_id) => {
                self.mutable_locals.insert(*local_id);
            }
            PatternKind::Tuple(items) | PatternKind::TupleStruct { items, .. } => {
                for &item in items {
                    self.record_mutable_pattern_bindings(item);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for field in fields {
                    self.record_mutable_pattern_bindings(field.pattern);
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

    fn check_pattern_root(&mut self, pattern_id: PatternId, expected: &Ty, context: &str) {
        let Some(actual) = self.pattern_root_ty(pattern_id) else {
            return;
        };

        if expected.compatible_with(&actual) {
            return;
        }

        self.diagnostics.push(
            Diagnostic::error(format!(
                "{context} has type mismatch: expected `{expected}`, found `{actual}`"
            ))
            .with_label(
                Label::new(self.module.pattern(pattern_id).span).with_message("pattern here"),
            ),
        );
    }

    fn invalid_tuple_struct_pattern_root_message(&self, pattern_id: PatternId) -> Option<String> {
        let pattern = self.module.pattern(pattern_id);
        let PatternKind::TupleStruct { path, .. } = &pattern.kind else {
            return None;
        };
        let resolution = self.resolution.pattern_resolution(pattern_id)?;
        match resolution {
            ValueResolution::Import(import_binding) => {
                local_item_for_import_binding(self.module, import_binding).and_then(|item_id| {
                    self.invalid_tuple_struct_pattern_item_path_message(item_id, path)
                })
            }
            ValueResolution::Item(item_id) => {
                self.invalid_tuple_struct_pattern_item_path_message(*item_id, path)
            }
            ValueResolution::Local(_)
            | ValueResolution::Param(_)
            | ValueResolution::SelfValue
            | ValueResolution::Function(_) => None,
        }
    }

    fn invalid_path_pattern_root_message(&self, pattern_id: PatternId) -> Option<String> {
        let pattern = self.module.pattern(pattern_id);
        let PatternKind::Path(path) = &pattern.kind else {
            return None;
        };
        let resolution = self.resolution.pattern_resolution(pattern_id)?;
        match resolution {
            ValueResolution::Import(import_binding) => {
                local_item_for_import_binding(self.module, import_binding)
                    .and_then(|item_id| self.invalid_path_pattern_item_path_message(item_id, path))
            }
            ValueResolution::Item(item_id) => {
                self.invalid_path_pattern_item_path_message(*item_id, path)
            }
            ValueResolution::Local(_)
            | ValueResolution::Param(_)
            | ValueResolution::SelfValue
            | ValueResolution::Function(_) => None,
        }
    }

    fn invalid_path_pattern_item_path_message(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        match &self.module.item(item_id).kind {
            ItemKind::Enum(_) if path.segments.len() == 2 => {
                match self.enum_variant_for_item_path(item_id, path) {
                    Ok(Some(variant)) => match &variant.fields {
                        VariantFields::Unit => None,
                        VariantFields::Tuple(_) | VariantFields::Struct(_) => Some(format!(
                            "path pattern syntax is not supported for `{path_text}`"
                        )),
                    },
                    Err(message) => Some(message),
                    Ok(None) => None,
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) if path.segments.len() == 1 => Some(format!(
                "path pattern syntax is not supported for `{path_text}`"
            )),
            ItemKind::Struct(_) | ItemKind::Enum(_) => None,
            ItemKind::Const(_) | ItemKind::Static(_) if path.segments.len() == 1 => self
                .const_item_path_pattern_ty(item_id)
                .is_none()
                .then(|| format!("path pattern syntax is not supported for `{path_text}`")),
            ItemKind::Const(_) | ItemKind::Static(_) => None,
            ItemKind::Function(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_)
                if path.segments.len() == 1 =>
            {
                Some(format!(
                    "path pattern syntax is not supported for `{path_text}`"
                ))
            }
            ItemKind::Function(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => None,
        }
    }

    fn invalid_tuple_struct_pattern_item_path_message(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        match &self.module.item(item_id).kind {
            ItemKind::Enum(_) if path.segments.len() == 2 => {
                match self.enum_variant_for_item_path(item_id, path) {
                    Ok(Some(variant)) => match &variant.fields {
                        VariantFields::Tuple(_) => None,
                        VariantFields::Struct(_) | VariantFields::Unit => Some(format!(
                            "tuple-struct pattern syntax is not supported for `{path_text}`"
                        )),
                    },
                    Err(message) => Some(message),
                    Ok(None) => None,
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) if path.segments.len() == 1 => Some(format!(
                "tuple-struct pattern syntax is not supported for `{path_text}`"
            )),
            ItemKind::Struct(_) | ItemKind::Enum(_) => None,
            ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_)
                if path.segments.len() == 1 =>
            {
                Some(format!(
                    "tuple-struct pattern syntax is not supported for `{path_text}`"
                ))
            }
            ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => None,
        }
    }

    fn invalid_struct_pattern_root_message(&self, pattern_id: PatternId) -> Option<String> {
        let pattern = self.module.pattern(pattern_id);
        let PatternKind::Struct { path, .. } = &pattern.kind else {
            return None;
        };
        let resolution = self.resolution.pattern_resolution(pattern_id)?;
        match resolution {
            ValueResolution::Import(import_binding) => {
                local_item_for_import_binding(self.module, import_binding).and_then(|item_id| {
                    self.invalid_struct_pattern_item_path_message(item_id, path)
                })
            }
            ValueResolution::Item(item_id) => {
                self.invalid_struct_pattern_item_path_message(*item_id, path)
            }
            ValueResolution::Local(_)
            | ValueResolution::Param(_)
            | ValueResolution::SelfValue
            | ValueResolution::Function(_) => None,
        }
    }

    fn invalid_struct_pattern_item_path_message(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        match &self.module.item(item_id).kind {
            ItemKind::Struct(_) if path.segments.len() == 1 => None,
            ItemKind::Struct(_) => None,
            ItemKind::Enum(_) if path.segments.len() == 2 => {
                match self.enum_variant_for_item_path(item_id, path) {
                    Ok(Some(variant)) => match &variant.fields {
                        VariantFields::Struct(_) => None,
                        VariantFields::Tuple(_) | VariantFields::Unit => Some(format!(
                            "struct pattern syntax is not supported for `{path_text}`"
                        )),
                    },
                    Err(message) => Some(message),
                    Ok(None) => None,
                }
            }
            ItemKind::Enum(_) if path.segments.len() == 1 => Some(format!(
                "struct pattern syntax is not supported for `{path_text}`"
            )),
            ItemKind::Enum(_) => None,
            ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_)
                if path.segments.len() == 1 =>
            {
                Some(format!(
                    "struct pattern syntax is not supported for `{path_text}`"
                ))
            }
            ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => None,
        }
    }

    fn check_literal_pattern(
        &mut self,
        pattern_id: PatternId,
        expected: &Ty,
        actual: &Ty,
        context: &str,
    ) {
        if expected.compatible_with(actual) {
            return;
        }

        self.diagnostics.push(
            Diagnostic::error(format!(
                "{context} has type mismatch: expected `{expected}`, found `{actual}`"
            ))
            .with_label(
                Label::new(self.module.pattern(pattern_id).span).with_message("pattern here"),
            ),
        );
    }

    fn pattern_root_ty(&self, pattern_id: PatternId) -> Option<Ty> {
        let pattern = self.module.pattern(pattern_id);
        let path = match &pattern.kind {
            PatternKind::Path(path)
            | PatternKind::TupleStruct { path, .. }
            | PatternKind::Struct { path, .. } => path,
            _ => return None,
        };

        let item_id = self
            .resolution
            .pattern_resolution(pattern_id)
            .and_then(|resolution| self.item_id_for_value_resolution(resolution))?;

        match &self.module.item(item_id).kind {
            ItemKind::Const(_) if path.segments.len() == 1 => {
                self.const_item_path_pattern_ty(item_id)
            }
            ItemKind::Struct(_) if path.segments.len() == 1 => Some(Ty::Item {
                item_id,
                name: item_display_name(self.module, item_id),
                args: Vec::new(),
            }),
            ItemKind::Enum(_) if path.segments.len() == 2 => Some(Ty::Item {
                item_id,
                name: item_display_name(self.module, item_id),
                args: Vec::new(),
            }),
            _ => None,
        }
    }

    fn tuple_struct_pattern_items(&self, pattern_id: PatternId) -> Option<Vec<Ty>> {
        let pattern = self.module.pattern(pattern_id);
        let PatternKind::TupleStruct { path, .. } = &pattern.kind else {
            return None;
        };

        let item_id = self
            .resolution
            .pattern_resolution(pattern_id)
            .and_then(|resolution| self.item_id_for_value_resolution(resolution))?;

        match &self.module.item(item_id).kind {
            ItemKind::Enum(_) if path.segments.len() == 2 => {
                match self.enum_variant_for_item_path(item_id, path).ok()? {
                    Some(variant) => match &variant.fields {
                        VariantFields::Tuple(types) => Some(
                            types
                                .iter()
                                .map(|&type_id| lower_type(self.module, self.resolution, type_id))
                                .collect(),
                        ),
                        _ => None,
                    },
                    None => None,
                }
            }
            _ => None,
        }
    }

    fn struct_pattern_fields(&self, pattern_id: PatternId) -> Option<Vec<FieldInfo>> {
        let pattern = self.module.pattern(pattern_id);
        let PatternKind::Struct { path, .. } = &pattern.kind else {
            return None;
        };

        let item_id = self
            .resolution
            .pattern_resolution(pattern_id)
            .and_then(|resolution| self.item_id_for_value_resolution(resolution))?;

        self.field_infos_for_item_path(item_id, path)
    }

    fn field_infos_for_item_path(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<Vec<FieldInfo>> {
        match &self.module.item(item_id).kind {
            ItemKind::Struct(struct_decl) if path.segments.len() == 1 => Some(
                struct_decl
                    .fields
                    .iter()
                    .map(|field| FieldInfo {
                        name: field.name.clone(),
                        ty: lower_type(self.module, self.resolution, field.ty),
                        has_default: field.default.is_some(),
                    })
                    .collect(),
            ),
            ItemKind::Enum(_) if path.segments.len() == 2 => {
                match self.enum_variant_for_item_path(item_id, path).ok()? {
                    Some(variant) => match &variant.fields {
                        VariantFields::Struct(fields) => Some(
                            fields
                                .iter()
                                .map(|field| FieldInfo {
                                    name: field.name.clone(),
                                    ty: lower_type(self.module, self.resolution, field.ty),
                                    has_default: field.default.is_some(),
                                })
                                .collect(),
                        ),
                        _ => None,
                    },
                    None => None,
                }
            }
            _ => None,
        }
    }

    fn unify_branch_types(&mut self, expr_id: ExprId, left: Ty, right: Ty, context: &str) -> Ty {
        if left.compatible_with(&right) {
            if left.is_unknown() { right } else { left }
        } else {
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "{context} must produce a consistent type, found `{left}` and `{right}`"
                ))
                .with_label(
                    Label::new(self.module.expr(expr_id).span).with_message("expression here"),
                ),
            );
            Ty::Unknown
        }
    }

    fn report_type_mismatch(&mut self, expr_id: ExprId, expected: &Ty, actual: &Ty, context: &str) {
        if expected.compatible_with(actual) {
            return;
        }

        self.diagnostics.push(
            Diagnostic::error(format!(
                "{context} has type mismatch: expected `{expected}`, found `{actual}`"
            ))
            .with_label(Label::new(self.module.expr(expr_id).span).with_message("expression here")),
        );
    }

    fn report_type_mismatch_expr(
        &mut self,
        expr_id: Option<ExprId>,
        expected: &Ty,
        actual: &Ty,
        context: &str,
    ) {
        let Some(expr_id) = expr_id else {
            return;
        };
        self.report_type_mismatch(expr_id, expected, actual, context);
    }

    fn report_type_mismatch_block(
        &mut self,
        block_id: BlockId,
        expected: &Ty,
        actual: &Ty,
        context: &str,
    ) {
        if expected.compatible_with(actual) {
            return;
        }

        self.diagnostics.push(
            Diagnostic::error(format!(
                "{context} has type mismatch: expected `{expected}`, found `{actual}`"
            ))
            .with_label(Label::new(self.module.block(block_id).span).with_message("block here")),
        );
    }

    fn report_type_mismatch_stmt(
        &mut self,
        stmt_id: ql_hir::StmtId,
        expected: &Ty,
        actual: &Ty,
        context: &str,
    ) {
        if expected.compatible_with(actual) {
            return;
        }

        self.diagnostics.push(
            Diagnostic::error(format!(
                "{context} has type mismatch: expected `{expected}`, found `{actual}`"
            ))
            .with_label(Label::new(self.module.stmt(stmt_id).span).with_message("statement here")),
        );
    }
}

#[derive(Clone)]
struct FieldInfo {
    name: String,
    ty: Ty,
    has_default: bool,
}

enum SelectedMember {
    Field { target: FieldTarget, ty: Ty },
    Method(MethodTarget),
    AmbiguousMethod,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MethodSource {
    Impl,
    Extend,
}

enum MethodSelection {
    None,
    Unique(MethodTarget),
    Ambiguous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssignmentTargetPolicy {
    EnforceValueType,
    SkipValueType,
}

struct Signature {
    is_async: bool,
    params: Vec<SignatureParam>,
    ret: Ty,
}

impl Signature {
    fn from_function(module: &Module, resolution: &ResolutionMap, function: &Function) -> Self {
        Self {
            is_async: function.is_async,
            params: function
                .params
                .iter()
                .filter_map(|param| match param {
                    Param::Regular(param) => Some(SignatureParam {
                        name: Some(param.name.clone()),
                        ty: lower_type(module, resolution, param.ty),
                    }),
                    Param::Receiver(_) => None,
                })
                .collect(),
            ret: function
                .return_type
                .map(|type_id| lower_type(module, resolution, type_id))
                .unwrap_or_else(void_ty),
        }
    }
}

struct SignatureParam {
    name: Option<String>,
    ty: Ty,
}

fn function_scope(function: &Function, resolution: &ResolutionMap) -> Option<ql_resolve::ScopeId> {
    function
        .params
        .iter()
        .find_map(|param| match param {
            Param::Regular(param) => resolution.type_scope(param.ty),
            Param::Receiver(_) => None,
        })
        .or_else(|| {
            function
                .return_type
                .and_then(|type_id| resolution.type_scope(type_id))
        })
}

fn op_text(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Assign => "=",
        BinaryOp::OrOr => "||",
        BinaryOp::AndAnd => "&&",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::EqEq => "==",
        BinaryOp::BangEq => "!=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
    }
}
