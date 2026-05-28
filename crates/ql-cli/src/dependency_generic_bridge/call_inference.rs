use std::collections::BTreeSet;

use ql_ast::{CallArg, Expr, ExprKind, FunctionDecl, TypeExpr, TypeExprKind};

use super::call_args::{call_arg_expr, ordered_dependency_generic_call_args};
use super::enum_bindings::EnumTypeBindings;
use super::expr_inference::infer_dependency_generic_expr_type;
use super::function_bindings::FunctionTypeBindings;
use super::inferred_type_conversion::inferred_type_from_type_expr_with_substitutions;
use super::inferred_types::InferredType;
use super::struct_bindings::StructTypeBindings;
use super::substitutions::{
    TypeSubstitutions, bind_generic_len_substitution, bind_generic_type_substitution,
    collect_generic_type_substitutions, generic_param_name_for_type_expr,
    type_expr_mentions_generic,
};
use super::value_bindings::ValueTypeBindings;

pub(super) fn infer_dependency_generic_function_substitutions(
    function: &FunctionDecl,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
) -> Option<TypeSubstitutions> {
    let ordered_args = ordered_dependency_generic_call_args(function, args)?;
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = TypeSubstitutions::new();
    {
        let mut collector = ArgSubstitutionCollector::new(
            &generic_names,
            bindings,
            function_bindings,
            enum_bindings,
            struct_bindings,
            &mut substitutions,
        );
        for (param_ty, arg) in ordered_args {
            if !type_expr_mentions_generic(param_ty, &generic_names) {
                continue;
            }
            if !collector.collect_arg(param_ty, arg) {
                return None;
            }
        }
    }
    if let (Some(return_ty), Some(expected_ty)) = (function.return_type.as_ref(), expected_ty)
        && type_expr_mentions_generic(return_ty, &generic_names)
    {
        let expected_ty = InferredType::from_type_expr(expected_ty)?;
        if !collect_generic_type_substitutions(
            return_ty,
            &expected_ty,
            &generic_names,
            &mut substitutions,
        ) {
            return None;
        }
    }
    Some(substitutions)
}

pub(super) fn collect_generic_type_substitutions_from_arg_expr(
    param_ty: &TypeExpr,
    arg: &CallArg,
    generic_names: &BTreeSet<&str>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    ArgSubstitutionCollector::new(
        generic_names,
        bindings,
        function_bindings,
        enum_bindings,
        struct_bindings,
        substitutions,
    )
    .collect_arg(param_ty, arg)
}

struct ArgSubstitutionCollector<'ctx, 'generic> {
    generic_names: &'ctx BTreeSet<&'generic str>,
    bindings: &'ctx ValueTypeBindings,
    function_bindings: &'ctx FunctionTypeBindings,
    enum_bindings: &'ctx EnumTypeBindings,
    struct_bindings: &'ctx StructTypeBindings,
    substitutions: &'ctx mut TypeSubstitutions,
}

impl<'ctx, 'generic> ArgSubstitutionCollector<'ctx, 'generic> {
    fn new(
        generic_names: &'ctx BTreeSet<&'generic str>,
        bindings: &'ctx ValueTypeBindings,
        function_bindings: &'ctx FunctionTypeBindings,
        enum_bindings: &'ctx EnumTypeBindings,
        struct_bindings: &'ctx StructTypeBindings,
        substitutions: &'ctx mut TypeSubstitutions,
    ) -> Self {
        Self {
            generic_names,
            bindings,
            function_bindings,
            enum_bindings,
            struct_bindings,
            substitutions,
        }
    }

    fn collect_arg(&mut self, param_ty: &TypeExpr, arg: &CallArg) -> bool {
        self.collect_expr(param_ty, call_arg_expr(arg))
    }

    fn collect_expr(&mut self, param_ty: &TypeExpr, expr: &Expr) -> bool {
        if let Some(generic_name) = generic_param_name_for_type_expr(param_ty, self.generic_names) {
            return self.bind_generic_from_expr(generic_name, expr);
        }

        match (&param_ty.kind, &expr.kind) {
            (
                TypeExprKind::Array {
                    element: param_element,
                    len: param_len,
                },
                ExprKind::Array(items),
            ) => self.collect_array_arg(param_element, param_len, items),
            (
                TypeExprKind::Array {
                    element: param_element,
                    len: param_len,
                },
                ExprKind::RepeatArray { value, len, .. },
            ) => self.collect_repeat_array_arg(param_element, param_len, value, len),
            (TypeExprKind::Tuple(param_items), ExprKind::Tuple(items))
                if param_items.len() == items.len() =>
            {
                self.collect_tuple_arg(param_items, items)
            }
            _ => self.collect_fallback_arg(param_ty, expr),
        }
    }

    fn bind_generic_from_expr(&mut self, generic_name: &str, expr: &Expr) -> bool {
        infer_dependency_generic_expr_type(
            expr,
            self.bindings,
            self.function_bindings,
            self.enum_bindings,
            self.struct_bindings,
        )
        .is_none_or(|arg_ty| {
            bind_generic_type_substitution(generic_name, &arg_ty, self.substitutions)
        })
    }

    fn collect_array_arg(
        &mut self,
        param_element: &TypeExpr,
        param_len: &str,
        items: &[Expr],
    ) -> bool {
        bind_generic_len_substitution(
            param_len,
            &items.len().to_string(),
            self.generic_names,
            self.substitutions,
        ) && items
            .iter()
            .all(|item| self.collect_expr(param_element, item))
    }

    fn collect_repeat_array_arg(
        &mut self,
        param_element: &TypeExpr,
        param_len: &str,
        value: &Expr,
        len: &str,
    ) -> bool {
        bind_generic_len_substitution(param_len, len, self.generic_names, self.substitutions)
            && self.collect_expr(param_element, value)
    }

    fn collect_tuple_arg(&mut self, param_items: &[TypeExpr], items: &[Expr]) -> bool {
        param_items
            .iter()
            .zip(items)
            .all(|(param_item, item)| self.collect_expr(param_item, item))
    }

    fn collect_fallback_arg(&mut self, param_ty: &TypeExpr, expr: &Expr) -> bool {
        infer_dependency_generic_expr_type(
            expr,
            self.bindings,
            self.function_bindings,
            self.enum_bindings,
            self.struct_bindings,
        )
        .is_none_or(|arg_ty| {
            collect_generic_type_substitutions(
                param_ty,
                &arg_ty,
                self.generic_names,
                self.substitutions,
            )
        })
    }
}

pub(super) fn infer_function_call_return_type(
    callee: &Expr,
    args: &[CallArg],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
) -> Option<InferredType> {
    let ExprKind::Name(name) = &callee.kind else {
        return None;
    };
    let function = function_bindings.get(name)?;
    let return_ty = function.return_type.as_ref()?;
    let substitutions = infer_dependency_generic_function_substitutions(
        function,
        args,
        None,
        bindings,
        function_bindings,
        enum_bindings,
        struct_bindings,
    )?;
    inferred_type_from_type_expr_with_substitutions(return_ty, &substitutions)
}

#[cfg(test)]
#[path = "call_inference_tests.rs"]
mod tests;
