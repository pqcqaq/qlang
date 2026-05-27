use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{
    self, BinaryOp, CallArg, Expr, ExprKind, FunctionDecl, Module, Param, Pattern, PatternKind,
    TypeExpr, TypeExprKind, UnaryOp,
};

use super::function_bindings::FunctionTypeBindings;
use super::inferred_types::{
    InferredType, InferredTypeKind, are_inferred_bool_types,
    inferred_type_from_type_expr_with_substitutions, is_inferred_bool_type,
    is_inferred_equality_comparable_type, is_inferred_numeric_type,
    is_inferred_ordered_comparable_type, render_inferred_tuple_type, type_expr_from_inferred_type,
};
use super::substitutions::{
    TypeSubstitutions, bind_generic_len_substitution, bind_generic_type_substitution,
    collect_generic_type_substitutions, generic_param_name_for_type_expr,
    type_expr_mentions_generic,
};

pub(super) type ValueTypeBindings = BTreeMap<String, InferredType>;

pub(super) fn collect_root_value_type_bindings(root_module: &Module) -> ValueTypeBindings {
    let mut bindings = ValueTypeBindings::new();
    for item in &root_module.items {
        let (ql_ast::ItemKind::Const(global) | ql_ast::ItemKind::Static(global)) = &item.kind
        else {
            continue;
        };
        if let Some(ty) = InferredType::from_type_expr(&global.ty) {
            bindings.insert(global.name.clone(), ty);
        }
    }
    bindings
}

pub(super) fn collect_function_param_type_bindings(
    function: &FunctionDecl,
    bindings: &mut ValueTypeBindings,
) {
    for param in &function.params {
        let Param::Regular { name, ty, .. } = param else {
            continue;
        };
        if let Some(ty) = InferredType::from_type_expr(ty) {
            bindings.insert(name.clone(), ty);
        }
    }
}

pub(super) fn collect_function_param_type_bindings_with_substitutions(
    function: &FunctionDecl,
    substitutions: &TypeSubstitutions,
    bindings: &mut ValueTypeBindings,
) {
    for param in &function.params {
        let Param::Regular { name, ty, .. } = param else {
            continue;
        };
        if let Some(ty) = inferred_type_from_type_expr_with_substitutions(ty, substitutions) {
            bindings.insert(name.clone(), ty);
        }
    }
}

pub(super) fn record_let_type_bindings(
    pattern: &Pattern,
    ty: Option<&TypeExpr>,
    value: &Expr,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) {
    if let Some(ty) = ty {
        record_pattern_type_bindings(pattern, ty, bindings);
        return;
    }
    if let PatternKind::Name(name) = &pattern.kind
        && let Some(ty) = infer_dependency_generic_expr_type(value, bindings, function_bindings)
    {
        bindings.insert(name.clone(), ty);
    }
}

fn record_pattern_type_bindings(
    pattern: &Pattern,
    ty: &TypeExpr,
    bindings: &mut ValueTypeBindings,
) {
    match (&pattern.kind, &ty.kind) {
        (PatternKind::Name(name), _) => {
            if let Some(ty) = InferredType::from_type_expr(ty) {
                bindings.insert(name.clone(), ty);
            }
        }
        (PatternKind::Tuple(patterns), TypeExprKind::Tuple(types))
            if patterns.len() == types.len() =>
        {
            for (pattern, ty) in patterns.iter().zip(types) {
                record_pattern_type_bindings(pattern, ty, bindings);
            }
        }
        _ => {}
    }
}

pub(super) fn infer_dependency_generic_function_substitutions(
    function: &FunctionDecl,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<TypeSubstitutions> {
    let ordered_args = ordered_dependency_generic_call_args(function, args)?;
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = TypeSubstitutions::new();
    for (param_ty, arg) in ordered_args {
        if !type_expr_mentions_generic(param_ty, &generic_names) {
            continue;
        }
        if !collect_generic_type_substitutions_from_arg_expr(
            param_ty,
            arg,
            &generic_names,
            bindings,
            function_bindings,
            &mut substitutions,
        ) {
            return None;
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

pub(super) fn call_arg_expr(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => expr,
    }
}

fn ordered_dependency_generic_call_args<'f, 'a>(
    function: &'f FunctionDecl,
    args: &'a [CallArg],
) -> Option<Vec<(&'f TypeExpr, &'a CallArg)>> {
    let regular_params = function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, ty, .. } => Some((name.as_str(), ty)),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>();
    if args.iter().all(|arg| matches!(arg, CallArg::Positional(_))) {
        return (regular_params.len() == args.len()).then(|| {
            regular_params
                .into_iter()
                .zip(args)
                .map(|((_, param_ty), arg)| (param_ty, arg))
                .collect()
        });
    }

    let mut ordered = vec![None; regular_params.len()];
    let mut next_positional = 0usize;
    let mut named_started = false;
    for arg in args {
        let index = match arg {
            CallArg::Named { name, .. } => {
                named_started = true;
                regular_params
                    .iter()
                    .position(|(param_name, _)| *param_name == name.as_str())?
            }
            CallArg::Positional(_) => {
                if named_started {
                    return None;
                }
                while next_positional < ordered.len() && ordered[next_positional].is_some() {
                    next_positional += 1;
                }
                if next_positional == ordered.len() {
                    return None;
                }
                next_positional
            }
        };
        if ordered[index].is_some() {
            return None;
        }
        ordered[index] = Some(arg);
    }

    regular_params
        .into_iter()
        .zip(ordered)
        .map(|((_, param_ty), arg)| arg.map(|arg| (param_ty, arg)))
        .collect()
}

pub(super) fn ordered_call_arg_expected_types(
    callee: &Expr,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Vec<Option<TypeExpr>> {
    let mut expected_types = vec![None; args.len()];
    let ExprKind::Name(name) = &callee.kind else {
        return expected_types;
    };
    let Some(function) = function_bindings.get(name) else {
        return expected_types;
    };
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = expected_ty
        .and_then(|expected_ty| {
            infer_dependency_generic_return_substitutions(function, expected_ty)
        })
        .unwrap_or_default();
    let Some(ordered_args) = ordered_dependency_generic_call_args(function, args) else {
        return expected_types;
    };

    for (param_ty, arg) in ordered_args {
        if !type_expr_mentions_generic(param_ty, &generic_names) {
            continue;
        }
        let _ = collect_generic_type_substitutions_from_arg_expr(
            param_ty,
            arg,
            &generic_names,
            bindings,
            function_bindings,
            &mut substitutions,
        );
    }

    let Some(ordered_args) = ordered_dependency_generic_call_args(function, args) else {
        return expected_types;
    };
    for (param_ty, arg) in ordered_args {
        let Some(source_index) = args
            .iter()
            .position(|candidate| std::ptr::eq(candidate, arg))
        else {
            continue;
        };
        expected_types[source_index] =
            inferred_type_from_type_expr_with_substitutions(param_ty, &substitutions)
                .map(|ty| type_expr_from_inferred_type(&ty))
                .or_else(|| Some(param_ty.clone()));
    }

    expected_types
}

fn infer_dependency_generic_return_substitutions(
    function: &FunctionDecl,
    expected_ty: &TypeExpr,
) -> Option<TypeSubstitutions> {
    let return_ty = function.return_type.as_ref()?;
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    if !type_expr_mentions_generic(return_ty, &generic_names) {
        return Some(TypeSubstitutions::new());
    }
    let expected_ty = InferredType::from_type_expr(expected_ty)?;
    let mut substitutions = TypeSubstitutions::new();
    collect_generic_type_substitutions(return_ty, &expected_ty, &generic_names, &mut substitutions)
        .then_some(substitutions)
}

fn collect_generic_type_substitutions_from_arg_expr(
    param_ty: &TypeExpr,
    arg: &CallArg,
    generic_names: &BTreeSet<&str>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    collect_generic_type_substitutions_from_expr(
        param_ty,
        call_arg_expr(arg),
        generic_names,
        bindings,
        function_bindings,
        substitutions,
    )
}

fn collect_generic_type_substitutions_from_expr(
    param_ty: &TypeExpr,
    expr: &Expr,
    generic_names: &BTreeSet<&str>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    if let Some(generic_name) = generic_param_name_for_type_expr(param_ty, generic_names) {
        return infer_dependency_generic_expr_type(expr, bindings, function_bindings).is_none_or(
            |arg_ty| bind_generic_type_substitution(generic_name, &arg_ty, substitutions),
        );
    }

    match (&param_ty.kind, &expr.kind) {
        (
            TypeExprKind::Array {
                element: param_element,
                len: param_len,
            },
            ExprKind::Array(items),
        ) => {
            bind_generic_len_substitution(
                param_len,
                &items.len().to_string(),
                generic_names,
                substitutions,
            ) && items.iter().all(|item| {
                collect_generic_type_substitutions_from_expr(
                    param_element,
                    item,
                    generic_names,
                    bindings,
                    function_bindings,
                    substitutions,
                )
            })
        }
        (
            TypeExprKind::Array {
                element: param_element,
                len: param_len,
            },
            ExprKind::RepeatArray { value, len, .. },
        ) => {
            bind_generic_len_substitution(param_len, len, generic_names, substitutions)
                && collect_generic_type_substitutions_from_expr(
                    param_element,
                    value,
                    generic_names,
                    bindings,
                    function_bindings,
                    substitutions,
                )
        }
        (TypeExprKind::Tuple(param_items), ExprKind::Tuple(items))
            if param_items.len() == items.len() =>
        {
            param_items.iter().zip(items).all(|(param_item, item)| {
                collect_generic_type_substitutions_from_expr(
                    param_item,
                    item,
                    generic_names,
                    bindings,
                    function_bindings,
                    substitutions,
                )
            })
        }
        _ => infer_dependency_generic_expr_type(expr, bindings, function_bindings).is_none_or(
            |arg_ty| {
                collect_generic_type_substitutions(param_ty, &arg_ty, generic_names, substitutions)
            },
        ),
    }
}

fn infer_dependency_generic_expr_type(
    expr: &Expr,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    match &expr.kind {
        ExprKind::Integer(_) => Some(InferredType::primitive("Int")),
        ExprKind::Bool(_) => Some(InferredType::primitive("Bool")),
        ExprKind::String { .. } => Some(InferredType::primitive("String")),
        ExprKind::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| infer_dependency_generic_expr_type(item, bindings, function_bindings))
                .collect::<Option<Vec<_>>>()?;
            Some(InferredType {
                rendered: render_inferred_tuple_type(&items),
                kind: InferredTypeKind::Tuple(items),
            })
        }
        ExprKind::Array(items) => {
            let (first, rest) = items.split_first()?;
            let element = infer_dependency_generic_expr_type(first, bindings, function_bindings)?;
            for item in rest {
                let item_ty =
                    infer_dependency_generic_expr_type(item, bindings, function_bindings)?;
                if item_ty != element {
                    return None;
                }
            }
            Some(InferredType {
                rendered: format!("[{}; {}]", element.rendered, items.len()),
                kind: InferredTypeKind::Array {
                    element: Box::new(element),
                    len: items.len().to_string(),
                },
            })
        }
        ExprKind::RepeatArray { value, len, .. } => {
            let element = infer_dependency_generic_expr_type(value, bindings, function_bindings)?;
            Some(InferredType {
                rendered: format!("[{}; {len}]", element.rendered),
                kind: InferredTypeKind::Array {
                    element: Box::new(element),
                    len: len.clone(),
                },
            })
        }
        ExprKind::Name(name) => bindings.get(name).cloned(),
        ExprKind::Block(block) | ExprKind::Unsafe(block) => {
            infer_dependency_generic_block_type(block, bindings, function_bindings)
        }
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            let then_ty =
                infer_dependency_generic_block_type(then_branch, bindings, function_bindings)?;
            let else_ty = infer_dependency_generic_expr_type(
                else_branch.as_deref()?,
                bindings,
                function_bindings,
            )?;
            (then_ty == else_ty).then_some(then_ty)
        }
        ExprKind::Match { arms, .. } => {
            let (first, rest) = arms.split_first()?;
            let first_ty =
                infer_dependency_generic_expr_type(&first.body, bindings, function_bindings)?;
            for arm in rest {
                let arm_ty =
                    infer_dependency_generic_expr_type(&arm.body, bindings, function_bindings)?;
                if arm_ty != first_ty {
                    return None;
                }
            }
            Some(first_ty)
        }
        ExprKind::Call { callee, args } => {
            infer_single_field_generic_variant_call_type(callee, args, bindings, function_bindings)
                .or_else(|| {
                    infer_function_call_return_type(callee, args, bindings, function_bindings)
                })
        }
        ExprKind::Bracket { target, items } => {
            infer_dependency_generic_projection_type(target, items, bindings, function_bindings)
        }
        ExprKind::Binary { left, op, right } => {
            let left = infer_dependency_generic_expr_type(left, bindings, function_bindings)?;
            let right = infer_dependency_generic_expr_type(right, bindings, function_bindings)?;
            infer_dependency_generic_binary_expr_type(*op, &left, &right)
        }
        ExprKind::Unary { op, expr } => {
            let expr = infer_dependency_generic_expr_type(expr, bindings, function_bindings)?;
            infer_dependency_generic_unary_expr_type(*op, &expr)
        }
        _ => None,
    }
}

fn infer_function_call_return_type(
    callee: &Expr,
    args: &[CallArg],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
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
    )?;
    inferred_type_from_type_expr_with_substitutions(return_ty, &substitutions)
}

fn infer_dependency_generic_block_type(
    block: &ql_ast::Block,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    let mut block_bindings = bindings.clone();
    for statement in &block.statements {
        if let ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } = &statement.kind
        {
            record_let_type_bindings(
                pattern,
                ty.as_ref(),
                value,
                &mut block_bindings,
                function_bindings,
            );
        }
    }
    infer_dependency_generic_expr_type(block.tail.as_deref()?, &block_bindings, function_bindings)
}

fn infer_dependency_generic_projection_type(
    target: &Expr,
    items: &[Expr],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    let [index] = items else {
        return None;
    };
    let target_ty = infer_dependency_generic_expr_type(target, bindings, function_bindings)?;
    match target_ty.kind {
        InferredTypeKind::Array { element, .. } => {
            let index_ty = infer_dependency_generic_expr_type(index, bindings, function_bindings)?;
            is_inferred_numeric_type(&index_ty).then_some(*element)
        }
        InferredTypeKind::Tuple(items) => {
            let ExprKind::Integer(index) = &index.kind else {
                return None;
            };
            items.get(ql_ast::parse_usize_literal(index)?).cloned()
        }
        _ => None,
    }
}

fn infer_dependency_generic_binary_expr_type(
    op: BinaryOp,
    left: &InferredType,
    right: &InferredType,
) -> Option<InferredType> {
    match op {
        BinaryOp::OrOr | BinaryOp::AndAnd if are_inferred_bool_types(left, right) => {
            Some(InferredType::primitive("Bool"))
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            if left == right && is_inferred_numeric_type(left) =>
        {
            Some(left.clone())
        }
        BinaryOp::EqEq | BinaryOp::BangEq
            if left == right && is_inferred_equality_comparable_type(left) =>
        {
            Some(InferredType::primitive("Bool"))
        }
        BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq
            if left == right && is_inferred_ordered_comparable_type(left) =>
        {
            Some(InferredType::primitive("Bool"))
        }
        BinaryOp::Assign => None,
        _ => None,
    }
}

fn infer_dependency_generic_unary_expr_type(
    op: UnaryOp,
    expr: &InferredType,
) -> Option<InferredType> {
    match op {
        UnaryOp::Not if is_inferred_bool_type(expr) => Some(InferredType::primitive("Bool")),
        UnaryOp::Neg if is_inferred_numeric_type(expr) => Some(expr.clone()),
        UnaryOp::Await | UnaryOp::Spawn => None,
        _ => None,
    }
}

fn infer_single_field_generic_variant_call_type(
    callee: &Expr,
    args: &[CallArg],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    let ExprKind::Member { object, .. } = &callee.kind else {
        return None;
    };
    let ExprKind::Name(type_name) = &object.kind else {
        return None;
    };
    let [CallArg::Positional(value)] = args else {
        return None;
    };
    let arg_ty = infer_dependency_generic_expr_type(value, bindings, function_bindings)?;
    Some(InferredType {
        rendered: format!("{type_name}[{}]", arg_ty.rendered),
        kind: InferredTypeKind::Named {
            path: vec![type_name.clone()],
            args: vec![arg_ty],
        },
    })
}

#[cfg(test)]
mod tests {
    use ql_ast::{ItemKind, StmtKind};

    use super::*;

    fn parse_module(source: &str) -> Module {
        ql_parser::parse_source(source).expect("test source should parse")
    }

    fn function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
        module
            .items
            .iter()
            .find_map(|item| match &item.kind {
                ItemKind::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .expect("test function should exist")
    }

    #[test]
    fn infers_block_tail_from_local_bindings_and_projection() {
        let module = parse_module(
            r#"
fn run() -> Int {
    let values: [Int; 3] = [1, 2, 3]
    let pair: (Int, Bool) = (values[0], true)
    {
        let local = pair[0]
        local
    }
}
"#,
        );
        let body = function(&module, "run")
            .body
            .as_ref()
            .expect("function should have a body")
            .clone();
        let expr = Expr::new(body.span, ExprKind::Block(body));

        let inferred = infer_dependency_generic_expr_type(
            &expr,
            &ValueTypeBindings::new(),
            &FunctionTypeBindings::new(),
        )
        .expect("block tail should infer");

        assert_eq!(inferred.rendered, "Int");
    }

    #[test]
    fn infers_array_type_and_length_substitutions_from_call_arguments() {
        let dependency = parse_module(
            r#"
package dep

pub fn reverse[T, N](values: [T; N]) -> [T; N] {
    return values
}
"#,
        );
        let root = parse_module(
            r#"
use dep.reverse as reverse

fn hidden() -> Int {
    return 2
}

fn run() -> [Int; 3] {
    return reverse([1, hidden(), 3])
}
"#,
        );
        let run = function(&root, "run");
        let StmtKind::Return(Some(expr)) = &run
            .body
            .as_ref()
            .expect("run should have a body")
            .statements[0]
            .kind
        else {
            panic!("run should return a call");
        };
        let ExprKind::Call { args, .. } = &expr.kind else {
            panic!("return expression should be a call");
        };

        let substitutions = infer_dependency_generic_function_substitutions(
            function(&dependency, "reverse"),
            args,
            run.return_type.as_ref(),
            &ValueTypeBindings::new(),
            &FunctionTypeBindings::new(),
        )
        .expect("call substitutions should infer");

        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
        assert_eq!(substitutions.get("N").map(String::as_str), Some("3"));
    }
}
