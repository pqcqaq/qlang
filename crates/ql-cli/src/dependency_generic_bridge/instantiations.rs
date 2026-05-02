use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{self, CallArg, Expr, ExprKind, FunctionDecl, ItemKind, Module, Param};

pub(super) fn collect_public_function_instantiations(
    root_module: &Module,
    module_import_path: &[String],
    function: &FunctionDecl,
) -> BTreeSet<BTreeMap<String, String>> {
    let local_names =
        dependency_imported_local_names(root_module, module_import_path, function.name.as_str());
    if local_names.is_empty() {
        return BTreeSet::new();
    }

    let mut instantiations = BTreeSet::new();
    for item in &root_module.items {
        collect_dependency_generic_function_instantiations_from_item(
            item,
            &local_names,
            function,
            &mut instantiations,
        );
    }
    instantiations
}

fn dependency_imported_local_names(
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
    instantiations: &mut BTreeSet<BTreeMap<String, String>>,
) {
    match &item.kind {
        ItemKind::Function(root_function) => {
            if let Some(body) = &root_function.body {
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    dependency_function,
                    instantiations,
                );
            }
        }
        ItemKind::Const(global) | ItemKind::Static(global) => {
            collect_dependency_generic_function_instantiations_from_expr(
                &global.value,
                local_names,
                dependency_function,
                instantiations,
            );
        }
        ItemKind::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                if let Some(default) = &field.default {
                    collect_dependency_generic_function_instantiations_from_expr(
                        default,
                        local_names,
                        dependency_function,
                        instantiations,
                    );
                }
            }
        }
        ItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                if let Some(body) = &method.body {
                    collect_dependency_generic_function_instantiations_from_block(
                        body,
                        local_names,
                        dependency_function,
                        instantiations,
                    );
                }
            }
        }
        ItemKind::Impl(impl_block) => {
            for method in &impl_block.methods {
                if let Some(body) = &method.body {
                    collect_dependency_generic_function_instantiations_from_block(
                        body,
                        local_names,
                        dependency_function,
                        instantiations,
                    );
                }
            }
        }
        ItemKind::Extend(extend_block) => {
            for method in &extend_block.methods {
                if let Some(body) = &method.body {
                    collect_dependency_generic_function_instantiations_from_block(
                        body,
                        local_names,
                        dependency_function,
                        instantiations,
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
    instantiations: &mut BTreeSet<BTreeMap<String, String>>,
) {
    for statement in &block.statements {
        match &statement.kind {
            ql_ast::StmtKind::Let { value, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    local_names,
                    function,
                    instantiations,
                );
            }
            ql_ast::StmtKind::Return(Some(value))
            | ql_ast::StmtKind::Defer(value)
            | ql_ast::StmtKind::Expr { expr: value, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    local_names,
                    function,
                    instantiations,
                );
            }
            ql_ast::StmtKind::While { condition, body } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    condition,
                    local_names,
                    function,
                    instantiations,
                );
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    instantiations,
                );
            }
            ql_ast::StmtKind::Loop { body } => {
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    instantiations,
                );
            }
            ql_ast::StmtKind::For { iterable, body, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    iterable,
                    local_names,
                    function,
                    instantiations,
                );
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    instantiations,
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
            local_names,
            function,
            instantiations,
        );
    }
}

fn collect_dependency_generic_function_instantiations_from_expr(
    expr: &Expr,
    local_names: &BTreeSet<String>,
    function: &FunctionDecl,
    instantiations: &mut BTreeSet<BTreeMap<String, String>>,
) {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::Name(name) = &callee.kind
                && local_names.contains(name)
                && let Some(substitutions) =
                    infer_dependency_generic_function_substitutions(function, args)
            {
                instantiations.insert(substitutions);
            }
            collect_dependency_generic_function_instantiations_from_expr(
                callee,
                local_names,
                function,
                instantiations,
            );
            for arg in args {
                match arg {
                    CallArg::Positional(value) | CallArg::Named { value, .. } => {
                        collect_dependency_generic_function_instantiations_from_expr(
                            value,
                            local_names,
                            function,
                            instantiations,
                        );
                    }
                }
            }
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => {
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    local_names,
                    function,
                    instantiations,
                );
            }
        }
        ExprKind::StructLiteral { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_dependency_generic_function_instantiations_from_expr(
                        value,
                        local_names,
                        function,
                        instantiations,
                    );
                }
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                left,
                local_names,
                function,
                instantiations,
            );
            collect_dependency_generic_function_instantiations_from_expr(
                right,
                local_names,
                function,
                instantiations,
            );
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_generic_function_instantiations_from_expr(
                expr,
                local_names,
                function,
                instantiations,
            );
        }
        ExprKind::Member { object, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                object,
                local_names,
                function,
                instantiations,
            );
        }
        ExprKind::Bracket { target, items } => {
            collect_dependency_generic_function_instantiations_from_expr(
                target,
                local_names,
                function,
                instantiations,
            );
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    local_names,
                    function,
                    instantiations,
                );
            }
        }
        ExprKind::Block(block) | ExprKind::Unsafe(block) => {
            collect_dependency_generic_function_instantiations_from_block(
                block,
                local_names,
                function,
                instantiations,
            );
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependency_generic_function_instantiations_from_expr(
                condition,
                local_names,
                function,
                instantiations,
            );
            collect_dependency_generic_function_instantiations_from_block(
                then_branch,
                local_names,
                function,
                instantiations,
            );
            if let Some(else_branch) = else_branch {
                collect_dependency_generic_function_instantiations_from_expr(
                    else_branch,
                    local_names,
                    function,
                    instantiations,
                );
            }
        }
        ExprKind::Match { value, arms } => {
            collect_dependency_generic_function_instantiations_from_expr(
                value,
                local_names,
                function,
                instantiations,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_dependency_generic_function_instantiations_from_expr(
                        guard,
                        local_names,
                        function,
                        instantiations,
                    );
                }
                collect_dependency_generic_function_instantiations_from_expr(
                    &arm.body,
                    local_names,
                    function,
                    instantiations,
                );
            }
        }
        ExprKind::Closure { body, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                body,
                local_names,
                function,
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
) -> Option<BTreeMap<String, String>> {
    if args.iter().any(|arg| matches!(arg, CallArg::Named { .. })) {
        return None;
    }
    let regular_params = function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { ty, .. } => Some(ty),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>();
    if regular_params.len() != args.len() {
        return None;
    }
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = BTreeMap::new();
    for (param_ty, arg) in regular_params.into_iter().zip(args) {
        let Some(generic_name) = generic_param_name_for_type_expr(param_ty, &generic_names) else {
            continue;
        };
        let arg_ty = infer_dependency_generic_literal_type(arg)?;
        match substitutions.get(generic_name) {
            Some(existing) if existing != &arg_ty => return None,
            Some(_) => {}
            None => {
                substitutions.insert(generic_name.to_owned(), arg_ty);
            }
        }
    }
    Some(substitutions)
}

fn generic_param_name_for_type_expr<'a>(
    ty: &ql_ast::TypeExpr,
    generic_names: &BTreeSet<&'a str>,
) -> Option<&'a str> {
    let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let [name] = path.segments.as_slice() else {
        return None;
    };
    generic_names.get(name.as_str()).copied()
}

fn infer_dependency_generic_literal_type(arg: &CallArg) -> Option<String> {
    let expr = match arg {
        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => expr,
    };
    match &expr.kind {
        ExprKind::Integer(_) => Some("Int".to_owned()),
        ExprKind::Bool(_) => Some("Bool".to_owned()),
        ExprKind::String { .. } => Some("String".to_owned()),
        _ => None,
    }
}
