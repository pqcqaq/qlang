use std::collections::BTreeSet;

use ql_ast::{self, FunctionDecl, ItemKind, TypeExpr};
use ql_span::Span;

use super::expr_inference::{
    ValueTypeBindings, collect_function_param_type_bindings, record_let_type_bindings,
};
use super::function_bindings::FunctionTypeBindings;
use super::instantiation_expr_scanner::collect_dependency_generic_function_instantiations_from_expr;
use super::substitutions::TypeSubstitutions;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiation {
    pub(super) callee_span: Span,
    pub(super) substitutions: TypeSubstitutions,
}

pub(super) fn collect_dependency_generic_function_instantiations_from_item(
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

pub(super) fn collect_dependency_generic_function_instantiations_from_block(
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
