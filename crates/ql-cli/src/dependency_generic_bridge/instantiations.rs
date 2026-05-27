use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{
    self, BinaryOp, CallArg, Expr, ExprKind, FunctionDecl, ItemKind, Module, Param, Pattern,
    PatternKind, TypeExpr, TypeExprKind, UnaryOp,
};
use ql_span::Span;

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

type ValueTypeBindings = BTreeMap<String, InferredType>;
pub(super) type FunctionTypeBindings = BTreeMap<String, FunctionDecl>;

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

pub(super) fn collect_imported_function_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> FunctionTypeBindings {
    let mut bindings = FunctionTypeBindings::new();
    for item in &dependency_module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        for local_name in
            dependency_imported_local_names(root_module, module_import_path, function.name.as_str())
        {
            bindings.insert(local_name, function.clone());
        }
    }
    bindings
}

pub(super) fn collect_local_function_type_bindings(root_module: &Module) -> FunctionTypeBindings {
    let mut bindings = FunctionTypeBindings::new();
    for item in &root_module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        bindings.insert(function.name.clone(), function.clone());
    }
    bindings
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

fn collect_root_value_type_bindings(root_module: &Module) -> ValueTypeBindings {
    let mut bindings = ValueTypeBindings::new();
    for item in &root_module.items {
        let (ItemKind::Const(global) | ItemKind::Static(global)) = &item.kind else {
            continue;
        };
        if let Some(ty) = InferredType::from_type_expr(&global.ty) {
            bindings.insert(global.name.clone(), ty);
        }
    }
    bindings
}

pub(super) fn dependency_imported_local_names(
    root_module: &Module,
    module_import_path: &[String],
    symbol_name: &str,
) -> BTreeSet<String> {
    let mut local_names = BTreeSet::new();
    let mut full_symbol_path = module_import_path.to_vec();
    full_symbol_path.push(symbol_name.to_owned());

    for use_decl in &root_module.uses {
        if let Some(group) = &use_decl.group {
            if use_decl.prefix.segments != module_import_path {
                continue;
            }
            for item in group {
                if item.name == symbol_name {
                    local_names.insert(item.alias.clone().unwrap_or_else(|| item.name.clone()));
                }
            }
            continue;
        }

        if use_decl.prefix.segments == full_symbol_path {
            local_names.insert(
                use_decl
                    .alias
                    .clone()
                    .unwrap_or_else(|| symbol_name.to_owned()),
            );
        } else if use_decl.prefix.segments == module_import_path {
            local_names.insert(symbol_name.to_owned());
        }
    }

    local_names
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

fn infer_dependency_generic_function_substitutions(
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

fn call_arg_expr(arg: &CallArg) -> &Expr {
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

fn ordered_call_arg_expected_types(
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

fn collect_function_param_type_bindings(function: &FunctionDecl, bindings: &mut ValueTypeBindings) {
    for param in &function.params {
        let Param::Regular { name, ty, .. } = param else {
            continue;
        };
        if let Some(ty) = InferredType::from_type_expr(ty) {
            bindings.insert(name.clone(), ty);
        }
    }
}

fn collect_function_param_type_bindings_with_substitutions(
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

fn record_let_type_bindings(
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
#[path = "instantiations_tests.rs"]
mod tests;
