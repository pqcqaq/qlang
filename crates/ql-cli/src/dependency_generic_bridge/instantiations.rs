use std::collections::BTreeSet;

use ql_ast::{self, Expr, ExprKind, FunctionDecl, ItemKind, Module, TypeExpr};
use ql_span::Span;

use super::expr_inference::{
    ValueTypeBindings, call_arg_expr, collect_function_param_type_bindings,
    collect_function_param_type_bindings_with_substitutions, collect_root_value_type_bindings,
    infer_dependency_generic_function_substitutions, ordered_call_arg_expected_types,
    record_let_type_bindings,
};
use super::function_bindings::{FunctionTypeBindings, dependency_imported_local_names};
use super::substitutions::TypeSubstitutions;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiation {
    pub(super) callee_span: Span,
    pub(super) substitutions: TypeSubstitutions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiations {
    pub(super) saw_call: bool,
    pub(super) instantiations: Vec<PublicFunctionCallInstantiation>,
}

#[cfg(test)]
fn collect_public_function_instantiations(
    root_module: &Module,
    module_import_path: &[String],
    function: &FunctionDecl,
) -> BTreeSet<TypeSubstitutions> {
    let function_bindings = FunctionTypeBindings::new();
    collect_public_function_call_instantiations(
        root_module,
        module_import_path,
        function,
        &function_bindings,
    )
    .into_iter()
    .map(|instantiation| instantiation.substitutions)
    .collect()
}

#[cfg(test)]
pub(super) fn collect_public_function_call_instantiations(
    root_module: &Module,
    module_import_path: &[String],
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    collect_public_function_call_instantiation_status(
        root_module,
        module_import_path,
        function,
        function_bindings,
    )
    .instantiations
}

pub(super) fn collect_public_function_call_instantiation_status(
    root_module: &Module,
    module_import_path: &[String],
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
) -> PublicFunctionCallInstantiations {
    let local_names =
        dependency_imported_local_names(root_module, module_import_path, function.name.as_str());
    if local_names.is_empty() {
        return PublicFunctionCallInstantiations {
            saw_call: false,
            instantiations: Vec::new(),
        };
    }

    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut saw_call = false;
    let mut instantiations = Vec::new();
    for item in &root_module.items {
        collect_dependency_generic_function_instantiations_from_item(
            item,
            &local_names,
            function,
            &root_bindings,
            function_bindings,
            &mut saw_call,
            &mut instantiations,
        );
    }
    PublicFunctionCallInstantiations {
        saw_call,
        instantiations,
    }
}

pub(super) fn collect_local_function_call_instantiations(
    root_module: &Module,
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let local_names = BTreeSet::from([function.name.clone()]);
    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut saw_call = false;
    let mut instantiations = Vec::new();
    for item in &root_module.items {
        let ItemKind::Function(root_function) = &item.kind else {
            collect_dependency_generic_function_instantiations_from_item(
                item,
                &local_names,
                function,
                &root_bindings,
                function_bindings,
                &mut saw_call,
                &mut instantiations,
            );
            continue;
        };
        if root_function.generics.is_empty() {
            collect_dependency_generic_function_instantiations_from_item(
                item,
                &local_names,
                function,
                &root_bindings,
                function_bindings,
                &mut saw_call,
                &mut instantiations,
            );
        }
    }
    instantiations
}

pub(super) fn collect_specialized_body_call_instantiations(
    caller_function: &FunctionDecl,
    target_function: &FunctionDecl,
    caller_substitutions: &TypeSubstitutions,
    function_bindings: &FunctionTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let local_names = BTreeSet::from([target_function.name.clone()]);
    collect_specialized_body_call_instantiations_for_local_names(
        caller_function,
        target_function,
        &local_names,
        caller_substitutions,
        function_bindings,
    )
}

pub(super) fn collect_specialized_body_call_instantiations_for_local_names(
    caller_function: &FunctionDecl,
    target_function: &FunctionDecl,
    local_names: &BTreeSet<String>,
    caller_substitutions: &TypeSubstitutions,
    function_bindings: &FunctionTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let Some(body) = &caller_function.body else {
        return Vec::new();
    };
    let mut bindings = ValueTypeBindings::new();
    collect_function_param_type_bindings_with_substitutions(
        caller_function,
        caller_substitutions,
        &mut bindings,
    );
    let mut instantiations = Vec::new();
    let mut saw_call = false;
    collect_dependency_generic_function_instantiations_from_block(
        body,
        local_names,
        target_function,
        &mut bindings,
        function_bindings,
        &mut saw_call,
        &mut instantiations,
        None,
        None,
    );
    instantiations
}

fn collect_dependency_generic_function_instantiations_from_item(
    item: &ql_ast::Item,
    local_names: &BTreeSet<String>,
    dependency_function: &FunctionDecl,
    root_bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    saw_call: &mut bool,
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
) {
    match &item.kind {
        ItemKind::Function(root_function) => {
            if let Some(body) = &root_function.body {
                let mut bindings = root_bindings.clone();
                collect_function_param_type_bindings(root_function, &mut bindings);
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    dependency_function,
                    &mut bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                    root_function.return_type.as_ref(),
                    root_function.return_type.as_ref(),
                );
            }
        }
        ItemKind::Const(global) | ItemKind::Static(global) => {
            collect_dependency_generic_function_instantiations_from_expr(
                &global.value,
                Some(&global.ty),
                None,
                local_names,
                dependency_function,
                root_bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ItemKind::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                if let Some(default) = &field.default {
                    collect_dependency_generic_function_instantiations_from_expr(
                        default,
                        Some(&field.ty),
                        None,
                        local_names,
                        dependency_function,
                        root_bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                    );
                }
            }
        }
        ItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                if let Some(body) = &method.body {
                    let mut bindings = root_bindings.clone();
                    collect_function_param_type_bindings(method, &mut bindings);
                    collect_dependency_generic_function_instantiations_from_block(
                        body,
                        local_names,
                        dependency_function,
                        &mut bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                        method.return_type.as_ref(),
                        method.return_type.as_ref(),
                    );
                }
            }
        }
        ItemKind::Impl(impl_block) => {
            for method in &impl_block.methods {
                if let Some(body) = &method.body {
                    let mut bindings = root_bindings.clone();
                    collect_function_param_type_bindings(method, &mut bindings);
                    collect_dependency_generic_function_instantiations_from_block(
                        body,
                        local_names,
                        dependency_function,
                        &mut bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                        method.return_type.as_ref(),
                        method.return_type.as_ref(),
                    );
                }
            }
        }
        ItemKind::Extend(extend_block) => {
            for method in &extend_block.methods {
                if let Some(body) = &method.body {
                    let mut bindings = root_bindings.clone();
                    collect_function_param_type_bindings(method, &mut bindings);
                    collect_dependency_generic_function_instantiations_from_block(
                        body,
                        local_names,
                        dependency_function,
                        &mut bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                        method.return_type.as_ref(),
                        method.return_type.as_ref(),
                    );
                }
            }
        }
        ItemKind::Enum(_) | ItemKind::TypeAlias(_) | ItemKind::ExternBlock(_) => {}
    }
}

fn collect_dependency_generic_function_instantiations_from_block(
    block: &ql_ast::Block,
    local_names: &BTreeSet<String>,
    function: &FunctionDecl,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    saw_call: &mut bool,
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
    return_expected_ty: Option<&TypeExpr>,
    tail_expected_ty: Option<&TypeExpr>,
) {
    for statement in &block.statements {
        match &statement.kind {
            ql_ast::StmtKind::Let {
                pattern, ty, value, ..
            } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    ty.as_ref(),
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
                record_let_type_bindings(pattern, ty.as_ref(), value, bindings, function_bindings);
            }
            ql_ast::StmtKind::Return(Some(value)) => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    return_expected_ty,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
            ql_ast::StmtKind::Defer(value) | ql_ast::StmtKind::Expr { expr: value, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
            ql_ast::StmtKind::While { condition, body } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    condition,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                    return_expected_ty,
                    None,
                );
            }
            ql_ast::StmtKind::Loop { body } => {
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                    return_expected_ty,
                    None,
                );
            }
            ql_ast::StmtKind::For { iterable, body, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    iterable,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                    return_expected_ty,
                    None,
                );
            }
            ql_ast::StmtKind::Return(None)
            | ql_ast::StmtKind::Break
            | ql_ast::StmtKind::Continue => {}
        }
    }
    if let Some(tail) = &block.tail {
        collect_dependency_generic_function_instantiations_from_expr(
            tail,
            tail_expected_ty,
            return_expected_ty,
            local_names,
            function,
            bindings,
            function_bindings,
            saw_call,
            instantiations,
        );
    }
}

fn collect_dependency_generic_function_instantiations_from_expr(
    expr: &Expr,
    expected_ty: Option<&TypeExpr>,
    return_expected_ty: Option<&TypeExpr>,
    local_names: &BTreeSet<String>,
    function: &FunctionDecl,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    saw_call: &mut bool,
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
) {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            let ordered_arg_expected_types = ordered_call_arg_expected_types(
                callee,
                args,
                expected_ty,
                bindings,
                function_bindings,
            );
            if let ExprKind::Name(name) = &callee.kind
                && local_names.contains(name)
            {
                *saw_call = true;
                if let Some(substitutions) = infer_dependency_generic_function_substitutions(
                    function,
                    args,
                    expected_ty,
                    bindings,
                    function_bindings,
                ) {
                    instantiations.push(PublicFunctionCallInstantiation {
                        callee_span: callee.span,
                        substitutions,
                    });
                }
            }
            collect_dependency_generic_function_instantiations_from_expr(
                callee,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            for (arg, arg_expected_ty) in args.iter().zip(ordered_arg_expected_types.iter()) {
                collect_dependency_generic_function_instantiations_from_expr(
                    call_arg_expr(arg),
                    arg_expected_ty.as_ref(),
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => {
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::RepeatArray { value, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                value,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::StructLiteral { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_dependency_generic_function_instantiations_from_expr(
                        value,
                        None,
                        return_expected_ty,
                        local_names,
                        function,
                        bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                    );
                }
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                left,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            collect_dependency_generic_function_instantiations_from_expr(
                right,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_generic_function_instantiations_from_expr(
                expr,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Member { object, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                object,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Bracket { target, items } => {
            collect_dependency_generic_function_instantiations_from_expr(
                target,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Block(block) | ExprKind::Unsafe(block) => {
            let mut block_bindings = bindings.clone();
            collect_dependency_generic_function_instantiations_from_block(
                block,
                local_names,
                function,
                &mut block_bindings,
                function_bindings,
                saw_call,
                instantiations,
                return_expected_ty,
                expected_ty,
            );
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependency_generic_function_instantiations_from_expr(
                condition,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            let mut then_bindings = bindings.clone();
            collect_dependency_generic_function_instantiations_from_block(
                then_branch,
                local_names,
                function,
                &mut then_bindings,
                function_bindings,
                saw_call,
                instantiations,
                return_expected_ty,
                expected_ty,
            );
            if let Some(else_branch) = else_branch {
                collect_dependency_generic_function_instantiations_from_expr(
                    else_branch,
                    expected_ty,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Match { value, arms } => {
            collect_dependency_generic_function_instantiations_from_expr(
                value,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_dependency_generic_function_instantiations_from_expr(
                        guard,
                        None,
                        return_expected_ty,
                        local_names,
                        function,
                        bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                    );
                }
                collect_dependency_generic_function_instantiations_from_expr(
                    &arm.body,
                    expected_ty,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Closure { body, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                body,
                None,
                None,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Integer(_)
        | ExprKind::String { .. }
        | ExprKind::Bool(_)
        | ExprKind::NoneLiteral
        | ExprKind::Name(_) => {}
    }
}

#[cfg(test)]
#[path = "instantiations_tests.rs"]
mod tests;
