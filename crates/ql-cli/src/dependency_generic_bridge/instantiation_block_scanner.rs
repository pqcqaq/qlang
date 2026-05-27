use std::collections::BTreeSet;

use ql_ast::{self, FunctionDecl, TypeExpr};

use super::function_bindings::FunctionTypeBindings;
use super::instantiation_expr_scanner::collect_dependency_generic_function_instantiations_from_expr;
use super::instantiation_scanner::PublicFunctionCallInstantiation;
use super::value_bindings::{ValueTypeBindings, record_let_type_bindings};

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
