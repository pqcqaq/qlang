use std::collections::{HashMap, HashSet};

use ql_ast::BinaryOp;
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{
    BlockId, CallArg, ExprId, ExprKind, Function, ItemId, ItemKind, LocalId, MatchArm, Module,
    Param, PatternId, PatternKind, StmtKind, VariantFields,
};
use ql_resolve::{ParamBinding, ResolutionMap, TypeResolution, ValueResolution};

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
        self.self_is_mutable = old_self_is_mutable;
        self.current_return = old_return;
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
            ExprKind::Call { callee, args } => self.check_call(expr_id, *callee, args),
            ExprKind::Member { object, field, .. } => self.check_member(expr_id, *object, field),
            ExprKind::Bracket { target, items } => self.check_bracket(expr_id, *target, items),
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
        if let Some(resolution) = self.resolution.expr_resolution(callee) {
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

        let root_ty = if let Some(item_id) = self
            .resolution
            .struct_literal_resolution(expr_id)
            .and_then(|resolution| self.item_id_for_type_resolution(resolution))
        {
            Ty::Item {
                item_id,
                name: item_display_name(self.module, item_id),
                args: Vec::new(),
            }
        } else {
            match self.resolution.struct_literal_resolution(expr_id) {
                Some(TypeResolution::Import(import_binding)) => Ty::Import {
                    path: import_binding.path.segments.join("."),
                    args: Vec::new(),
                },
                Some(TypeResolution::Builtin(builtin)) => Ty::Builtin(*builtin),
                Some(TypeResolution::Generic(_)) => Ty::Generic(path.segments.join(".")),
                None => Ty::Named {
                    path: path.segments.join("."),
                    args: Vec::new(),
                },
                Some(TypeResolution::Item(_)) => unreachable!("local items are handled above"),
            }
        };

        let Some(fields_info) = self
            .resolution
            .struct_literal_resolution(expr_id)
            .and_then(|resolution| self.item_id_for_type_resolution(resolution))
            .and_then(|item_id| self.field_infos_for_item_path(item_id, path))
        else {
            if let Some(message) = self.invalid_struct_literal_root_message(expr_id, path) {
                self.diagnostics.push(
                    Diagnostic::error(message)
                        .with_label(Label::new(expr.span).with_message("struct literal here")),
                );
            }
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

    fn invalid_struct_literal_root_message(
        &self,
        expr_id: ExprId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        let resolution = self.resolution.struct_literal_resolution(expr_id)?;
        match resolution {
            TypeResolution::Builtin(_) | TypeResolution::Generic(_) => Some(format!(
                "struct literal syntax is not supported for `{path_text}`"
            )),
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

    fn invalid_struct_literal_item_path_message(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        match &self.module.item(item_id).kind {
            ItemKind::Struct(_) if path.segments.len() == 1 => None,
            ItemKind::Struct(_) => Some(format!(
                "struct literal syntax is not supported for `{path_text}`"
            )),
            ItemKind::Enum(enum_decl) if path.segments.len() >= 2 => {
                let variant_name = path.segments.last()?;
                let variant = enum_decl
                    .variants
                    .iter()
                    .find(|variant| &variant.name == variant_name)?;
                match &variant.fields {
                    VariantFields::Struct(_) => None,
                    VariantFields::Tuple(_) | VariantFields::Unit => Some(format!(
                        "struct literal syntax is not supported for `{path_text}`"
                    )),
                }
            }
            ItemKind::Enum(_)
            | ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => Some(format!(
                "struct literal syntax is not supported for `{path_text}`"
            )),
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
        let expr = self.module.expr(expr_id);
        let unsupported_target = |checker: &mut Self, message: String| {
            checker.diagnostics.push(
                Diagnostic::error(message)
                    .with_label(Label::new(expr.span).with_message("assignment target here")),
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
                        .with_label(Label::new(expr.span).with_message("assignment target here")),
                    );
                    AssignmentTargetPolicy::EnforceValueType
                }
                Some(ValueResolution::Param(_)) => {
                    self.diagnostics.push(
                        Diagnostic::error(format!("cannot assign to immutable parameter `{name}`"))
                            .with_label(
                                Label::new(expr.span).with_message("assignment target here"),
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
                        .with_label(Label::new(expr.span).with_message("assignment target here")),
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
            ExprKind::Member { .. } => unsupported_target(
                self,
                "assignment through member access is not supported yet; only bare mutable bindings can be assigned"
                    .to_string(),
            ),
            ExprKind::Bracket { .. } => unsupported_target(
                self,
                "assignment through indexing is not supported yet; only bare mutable bindings can be assigned"
                    .to_string(),
            ),
            _ => unsupported_target(
                self,
                "this assignment target is not supported yet; only bare mutable bindings can be assigned"
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
                if let Some(message) = self.invalid_path_pattern_root_message(pattern_id) {
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
            ItemKind::Enum(enum_decl) if path.segments.len() >= 2 => {
                let variant_name = path.segments.last()?;
                let variant = enum_decl
                    .variants
                    .iter()
                    .find(|variant| &variant.name == variant_name)?;
                match &variant.fields {
                    VariantFields::Unit => None,
                    VariantFields::Tuple(_) | VariantFields::Struct(_) => Some(format!(
                        "path pattern syntax is not supported for `{path_text}`"
                    )),
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) => Some(format!(
                "path pattern syntax is not supported for `{path_text}`"
            )),
            ItemKind::Function(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => Some(format!(
                "path pattern syntax is not supported for `{path_text}`"
            )),
            ItemKind::Const(_) | ItemKind::Static(_) => None,
        }
    }

    fn invalid_tuple_struct_pattern_item_path_message(
        &self,
        item_id: ItemId,
        path: &ql_ast::Path,
    ) -> Option<String> {
        let path_text = path.segments.join(".");
        match &self.module.item(item_id).kind {
            ItemKind::Enum(enum_decl) if path.segments.len() >= 2 => {
                let variant_name = path.segments.last()?;
                let variant = enum_decl
                    .variants
                    .iter()
                    .find(|variant| &variant.name == variant_name)?;
                match &variant.fields {
                    VariantFields::Tuple(_) => None,
                    VariantFields::Struct(_) | VariantFields::Unit => Some(format!(
                        "tuple-struct pattern syntax is not supported for `{path_text}`"
                    )),
                }
            }
            ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => Some(format!(
                "tuple-struct pattern syntax is not supported for `{path_text}`"
            )),
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
            ItemKind::Struct(_) => Some(format!(
                "struct pattern syntax is not supported for `{path_text}`"
            )),
            ItemKind::Enum(enum_decl) if path.segments.len() >= 2 => {
                let variant_name = path.segments.last()?;
                let variant = enum_decl
                    .variants
                    .iter()
                    .find(|variant| &variant.name == variant_name)?;
                match &variant.fields {
                    VariantFields::Struct(_) => None,
                    VariantFields::Tuple(_) | VariantFields::Unit => Some(format!(
                        "struct pattern syntax is not supported for `{path_text}`"
                    )),
                }
            }
            ItemKind::Enum(_)
            | ItemKind::Function(_)
            | ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Trait(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Impl(_)
            | ItemKind::Extend(_)
            | ItemKind::ExternBlock(_) => Some(format!(
                "struct pattern syntax is not supported for `{path_text}`"
            )),
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
            ItemKind::Struct(_) if path.segments.len() == 1 => Some(Ty::Item {
                item_id,
                name: item_display_name(self.module, item_id),
                args: Vec::new(),
            }),
            ItemKind::Enum(_) if path.segments.len() >= 2 => Some(Ty::Item {
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
