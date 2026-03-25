use std::collections::{HashMap, HashSet};

use ql_ast::BinaryOp;
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{
    BlockId, CallArg, ExprId, ExprKind, Function, ItemKind, LocalId, MatchArm, Module, Param,
    PatternId, PatternKind, StmtKind, VariantFields,
};
use ql_resolve::{ParamBinding, ResolutionMap, TypeResolution, ValueResolution};

use crate::types::{Ty, item_display_name, lower_type, void_ty};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypingResult {
    pub diagnostics: Vec<Diagnostic>,
    pub expr_types: HashMap<ExprId, Ty>,
    pub pattern_types: HashMap<PatternId, Ty>,
    pub local_types: HashMap<LocalId, Ty>,
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
    }
}

struct Checker<'a> {
    module: &'a Module,
    resolution: &'a ResolutionMap,
    diagnostics: Vec<Diagnostic>,
    expr_types: HashMap<ExprId, Ty>,
    pattern_types: HashMap<PatternId, Ty>,
    local_types: HashMap<LocalId, Ty>,
    param_types: HashMap<ParamBinding, Ty>,
    self_type: Option<Ty>,
    current_return: Option<Ty>,
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
            param_types: HashMap::new(),
            self_type: None,
            current_return: None,
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
        let old_return = self.current_return.clone();
        let old_param_types = std::mem::take(&mut self.param_types);

        self.self_type = self_type;
        self.current_return = Some(
            function
                .return_type
                .map(|type_id| lower_type(self.module, self.resolution, type_id))
                .unwrap_or_else(void_ty),
        );

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
                self.report_type_mismatch_expr(
                    self.module.block(body).tail,
                    expected,
                    &actual,
                    "function body",
                );
            }
        }

        self.self_type = old_self;
        self.current_return = old_return;
        self.param_types = old_param_types;
    }

    fn check_block(&mut self, block_id: BlockId) -> Ty {
        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                StmtKind::Let { pattern, value, .. } => {
                    let value_ty = self.check_expr(*value, None);
                    self.bind_pattern(*pattern, &value_ty);
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
                StmtKind::Break | StmtKind::Continue => {}
                StmtKind::While { condition, body } => {
                    self.check_bool_condition(*condition, "while condition");
                    self.check_block(*body);
                }
                StmtKind::Loop { body } => {
                    self.check_block(*body);
                }
                StmtKind::For {
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    self.check_expr(*iterable, None);
                    self.bind_pattern(*pattern, &Ty::Unknown);
                    self.check_block(*body);
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
            ExprKind::Array(items) => {
                for &item in items {
                    self.check_expr(item, None);
                }
                Ty::Unknown
            }
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
            ExprKind::Call { callee, args } => self.check_call(expr_id, *callee, args),
            ExprKind::Member { object, field } => self.check_member(expr_id, *object, field),
            ExprKind::Bracket { target, items } => {
                self.check_expr(*target, None);
                for &item in items {
                    self.check_expr(item, None);
                }
                Ty::Unknown
            }
            ExprKind::StructLiteral { .. } => self.check_struct_literal(expr_id),
            ExprKind::Binary { left, op, right } => self.check_binary(expr_id, *left, *op, *right),
            ExprKind::Unary { expr, .. } => self.check_expr(*expr, None),
            ExprKind::Question(expr) => {
                self.check_expr(*expr, None);
                Ty::Unknown
            }
        };
        self.expr_types.insert(expr_id, ty.clone());
        ty
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
            Some(ValueResolution::Item(item_id)) => match &self.module.item(*item_id).kind {
                ItemKind::Const(global) | ItemKind::Static(global) => {
                    lower_type(self.module, self.resolution, global.ty)
                }
                _ => Ty::Unknown,
            },
            Some(ValueResolution::Import(_)) | None => Ty::Unknown,
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
        let body_ty = self.check_expr(body, expected_ret);
        if let Some(expected_ret) = expected_ret {
            self.report_type_mismatch(body, expected_ret, &body_ty, "closure body");
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
            ret: Box::new(body_ty),
        }
    }

    fn check_call(&mut self, expr_id: ExprId, callee: ExprId, args: &[CallArg]) -> Ty {
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
        signature.ret
    }

    fn check_member(&mut self, expr_id: ExprId, object: ExprId, field: &str) -> Ty {
        let object_ty = self.check_expr(object, None);
        let Ty::Item { item_id, .. } = &object_ty else {
            return Ty::Unknown;
        };

        if self.has_method_candidate(&object_ty, field) {
            return Ty::Unknown;
        }

        match &self.module.item(*item_id).kind {
            ItemKind::Struct(struct_decl) => {
                if let Some(field) = struct_decl
                    .fields
                    .iter()
                    .find(|candidate| candidate.name == field)
                {
                    lower_type(self.module, self.resolution, field.ty)
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "unknown member `{field}` on type `{object_ty}`"
                        ))
                        .with_label(
                            Label::new(self.module.expr(expr_id).span)
                                .with_message("member access here"),
                        ),
                    );
                    Ty::Unknown
                }
            }
            _ => Ty::Unknown,
        }
    }

    fn has_method_candidate(&self, object_ty: &Ty, field: &str) -> bool {
        self.module
            .items
            .iter()
            .copied()
            .any(|item_id| match &self.module.item(item_id).kind {
                ItemKind::Impl(impl_block) => {
                    let target_ty = lower_type(self.module, self.resolution, impl_block.target);
                    object_ty.compatible_with(&target_ty)
                        && impl_block.methods.iter().any(|method| method.name == field)
                }
                ItemKind::Extend(extend_block) => {
                    let target_ty = lower_type(self.module, self.resolution, extend_block.target);
                    object_ty.compatible_with(&target_ty)
                        && extend_block
                            .methods
                            .iter()
                            .any(|method| method.name == field)
                }
                _ => false,
            })
    }

    fn call_signature(&self, callee: ExprId, callee_ty: &Ty) -> Option<Signature> {
        if let Some(ValueResolution::Function(function_ref)) =
            self.resolution.expr_resolution(callee)
        {
            let function = self.module.function(*function_ref);
            return Some(Signature::from_function(
                self.module,
                self.resolution,
                function,
            ));
        }

        match callee_ty {
            Ty::Callable { params, ret } => Some(Signature {
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

    fn check_struct_literal(&mut self, expr_id: ExprId) -> Ty {
        let expr = self.module.expr(expr_id);
        let ExprKind::StructLiteral { path, fields } = &expr.kind else {
            return Ty::Unknown;
        };

        let root_ty = match self.resolution.struct_literal_resolution(expr_id) {
            Some(TypeResolution::Item(item_id)) => Ty::Item {
                item_id: *item_id,
                name: item_display_name(self.module, *item_id),
                args: Vec::new(),
            },
            Some(TypeResolution::Import(import_path)) => Ty::Import {
                path: import_path.segments.join("."),
                args: Vec::new(),
            },
            Some(TypeResolution::Builtin(builtin)) => Ty::Builtin(*builtin),
            Some(TypeResolution::Generic(_)) => Ty::Generic(path.segments.join(".")),
            None => Ty::Named {
                path: path.segments.join("."),
                args: Vec::new(),
            },
        };

        let Some(fields_info) = self.struct_field_map(&root_ty) else {
            for field in fields {
                self.check_expr(field.value, None);
            }
            return root_ty;
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

    fn check_binary(&mut self, expr_id: ExprId, left: ExprId, op: BinaryOp, right: ExprId) -> Ty {
        let left_ty = self.check_expr(left, None);
        let right_ty = self.check_expr(right, None);

        match op {
            BinaryOp::Assign => {
                self.report_type_mismatch(right, &left_ty, &right_ty, "assignment");
                Ty::Unknown
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if left_ty.is_unknown() && right_ty.is_unknown() {
                    return Ty::Unknown;
                }
                if left_ty.is_numeric()
                    && right_ty.is_numeric()
                    && left_ty.compatible_with(&right_ty)
                {
                    return if left_ty.is_unknown() {
                        right_ty
                    } else {
                        left_ty
                    };
                }
                if left_ty.is_unknown() && right_ty.is_numeric() {
                    return right_ty;
                }
                if right_ty.is_unknown() && left_ty.is_numeric() {
                    return left_ty;
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
                if (left_ty.is_numeric() || left_ty.is_unknown())
                    && (right_ty.is_numeric() || right_ty.is_unknown())
                {
                    Ty::Builtin(ql_resolve::BuiltinType::Bool)
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "comparison operator `{}` expects numeric operands, found `{}` and `{}`",
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
                self.check_pattern_root(pattern_id, expected, "tuple-struct pattern");
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
                    for &item in items {
                        self.bind_pattern(item, &Ty::Unknown);
                    }
                }
            }
            PatternKind::Struct { fields, .. } => {
                self.check_pattern_root(pattern_id, expected, "struct pattern");
                let field_types = self.struct_pattern_fields(pattern_id);
                for field in fields {
                    let field_ty = field_types
                        .as_ref()
                        .and_then(|field_types| {
                            field_types.iter().find(|info| info.name == field.name)
                        })
                        .map(|info| info.ty.clone())
                        .unwrap_or(Ty::Unknown);
                    self.bind_pattern(field.pattern, &field_ty);
                }
            }
            PatternKind::Path(_) => {
                self.check_pattern_root(pattern_id, expected, "path pattern");
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

        let Some(ValueResolution::Item(item_id)) = self.resolution.pattern_resolution(pattern_id)
        else {
            return None;
        };

        match &self.module.item(*item_id).kind {
            ItemKind::Struct(_) if path.segments.len() == 1 => Some(Ty::Item {
                item_id: *item_id,
                name: item_display_name(self.module, *item_id),
                args: Vec::new(),
            }),
            ItemKind::Enum(_) if path.segments.len() >= 2 => Some(Ty::Item {
                item_id: *item_id,
                name: item_display_name(self.module, *item_id),
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

        let Some(ValueResolution::Item(item_id)) = self.resolution.pattern_resolution(pattern_id)
        else {
            return None;
        };

        match &self.module.item(*item_id).kind {
            ItemKind::Enum(enum_decl) if path.segments.len() >= 2 => {
                let variant_name = path.segments.last()?;
                let variant = enum_decl
                    .variants
                    .iter()
                    .find(|variant| &variant.name == variant_name)?;
                match &variant.fields {
                    VariantFields::Tuple(types) => Some(
                        types
                            .iter()
                            .map(|&type_id| lower_type(self.module, self.resolution, type_id))
                            .collect(),
                    ),
                    _ => None,
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

        let Some(ValueResolution::Item(item_id)) = self.resolution.pattern_resolution(pattern_id)
        else {
            return None;
        };

        match &self.module.item(*item_id).kind {
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
            ItemKind::Enum(enum_decl) if path.segments.len() >= 2 => {
                let variant_name = path.segments.last()?;
                let variant = enum_decl
                    .variants
                    .iter()
                    .find(|variant| &variant.name == variant_name)?;
                match &variant.fields {
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
                }
            }
            _ => None,
        }
    }

    fn struct_field_map(&self, root_ty: &Ty) -> Option<Vec<FieldInfo>> {
        let Ty::Item { item_id, .. } = root_ty else {
            return None;
        };

        let ItemKind::Struct(struct_decl) = &self.module.item(*item_id).kind else {
            return None;
        };

        Some(
            struct_decl
                .fields
                .iter()
                .map(|field| FieldInfo {
                    name: field.name.clone(),
                    ty: lower_type(self.module, self.resolution, field.ty),
                    has_default: field.default.is_some(),
                })
                .collect(),
        )
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

struct Signature {
    params: Vec<SignatureParam>,
    ret: Ty,
}

impl Signature {
    fn from_function(module: &Module, resolution: &ResolutionMap, function: &Function) -> Self {
        Self {
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
