use ql_ast::{self, TypeExpr};

use super::instantiation_expr_scanner::collect_dependency_generic_function_instantiations_from_expr;
use super::instantiation_scan_context::InstantiationScanContext;
use super::value_bindings::{
    ValueTypeBindings, record_iterable_type_bindings, record_let_type_bindings_with_substitutions,
};

pub(super) fn collect_dependency_generic_function_instantiations_from_block(
    block: &ql_ast::Block,
    bindings: &mut ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
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
                    bindings,
                    context,
                );
                record_let_type_bindings_with_substitutions(
                    pattern,
                    ty.as_ref(),
                    value,
                    bindings,
                    context.function_bindings,
                    context.enum_bindings,
                    context.struct_bindings,
                    context.type_substitutions,
                );
            }
            ql_ast::StmtKind::Return(Some(value)) => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    return_expected_ty,
                    return_expected_ty,
                    bindings,
                    context,
                );
            }
            ql_ast::StmtKind::Defer(value) | ql_ast::StmtKind::Expr { expr: value, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    None,
                    return_expected_ty,
                    bindings,
                    context,
                );
            }
            ql_ast::StmtKind::While { condition, body } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    condition,
                    None,
                    return_expected_ty,
                    bindings,
                    context,
                );
                scan_nested_block(body, bindings, context, return_expected_ty);
            }
            ql_ast::StmtKind::Loop { body } => {
                scan_nested_block(body, bindings, context, return_expected_ty);
            }
            ql_ast::StmtKind::For {
                pattern,
                iterable,
                body,
                ..
            } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    iterable,
                    None,
                    return_expected_ty,
                    bindings,
                    context,
                );
                scan_for_block(
                    pattern,
                    iterable,
                    body,
                    bindings,
                    context,
                    return_expected_ty,
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
            bindings,
            context,
        );
    }
}

fn scan_for_block(
    pattern: &ql_ast::Pattern,
    iterable: &ql_ast::Expr,
    body: &ql_ast::Block,
    bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
    return_expected_ty: Option<&TypeExpr>,
) {
    let mut body_bindings = bindings.clone();
    record_iterable_type_bindings(
        pattern,
        iterable,
        &mut body_bindings,
        context.function_bindings,
        context.enum_bindings,
        context.struct_bindings,
    );
    collect_dependency_generic_function_instantiations_from_block(
        body,
        &mut body_bindings,
        context,
        return_expected_ty,
        None,
    );
}

fn scan_nested_block(
    block: &ql_ast::Block,
    bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
    return_expected_ty: Option<&TypeExpr>,
) {
    let mut body_bindings = bindings.clone();
    collect_dependency_generic_function_instantiations_from_block(
        block,
        &mut body_bindings,
        context,
        return_expected_ty,
        None,
    );
}
