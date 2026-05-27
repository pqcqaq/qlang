use ql_ast::{CallArg, Expr, ExprKind, TypeExpr};

use super::call_args::ordered_call_arg_expected_types;
use super::call_inference::infer_dependency_generic_function_substitutions;
use super::instantiation_scan_context::InstantiationScanContext;
use super::value_bindings::ValueTypeBindings;

pub(super) fn scan_call_instantiation(
    callee: &Expr,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
) -> Vec<Option<TypeExpr>> {
    let ordered_arg_expected_types = ordered_call_arg_expected_types(
        callee,
        args,
        expected_ty,
        bindings,
        context.function_bindings,
    );
    record_target_call_instantiation(callee, args, expected_ty, bindings, context);
    ordered_arg_expected_types
}

fn record_target_call_instantiation(
    callee: &Expr,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
) {
    let ExprKind::Name(name) = &callee.kind else {
        return;
    };
    if !context.local_names.contains(name) {
        return;
    }

    context.mark_call_seen();
    if let Some(substitutions) = infer_dependency_generic_function_substitutions(
        context.target_function,
        args,
        expected_ty,
        bindings,
        context.function_bindings,
    ) {
        context.push_instantiation(callee.span, substitutions);
    }
}
