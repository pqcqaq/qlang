use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{
    self, CallArg, Expr, ExprKind, FunctionDecl, ItemKind, Module, Param, Pattern, PatternKind,
    TypeExpr, TypeExprKind,
};

type TypeBindings = BTreeMap<String, String>;

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

    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut instantiations = BTreeSet::new();
    for item in &root_module.items {
        collect_dependency_generic_function_instantiations_from_item(
            item,
            &local_names,
            function,
            &root_bindings,
            &mut instantiations,
        );
    }
    instantiations
}

fn collect_root_value_type_bindings(root_module: &Module) -> TypeBindings {
    let mut bindings = TypeBindings::new();
    for item in &root_module.items {
        let (ItemKind::Const(global) | ItemKind::Static(global)) = &item.kind else {
            continue;
        };
        if let Some(ty) = primitive_type_name_for_type_expr(&global.ty) {
            bindings.insert(global.name.clone(), ty);
        }
    }
    bindings
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
    root_bindings: &TypeBindings,
    instantiations: &mut BTreeSet<TypeBindings>,
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
                    instantiations,
                );
            }
        }
        ItemKind::Const(global) | ItemKind::Static(global) => {
            collect_dependency_generic_function_instantiations_from_expr(
                &global.value,
                local_names,
                dependency_function,
                root_bindings,
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
                        root_bindings,
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
                        instantiations,
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
                        instantiations,
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
    bindings: &mut TypeBindings,
    instantiations: &mut BTreeSet<TypeBindings>,
) {
    for statement in &block.statements {
        match &statement.kind {
            ql_ast::StmtKind::Let {
                pattern, ty, value, ..
            } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    local_names,
                    function,
                    bindings,
                    instantiations,
                );
                record_let_type_bindings(pattern, ty.as_ref(), value, bindings);
            }
            ql_ast::StmtKind::Return(Some(value))
            | ql_ast::StmtKind::Defer(value)
            | ql_ast::StmtKind::Expr { expr: value, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    value,
                    local_names,
                    function,
                    bindings,
                    instantiations,
                );
            }
            ql_ast::StmtKind::While { condition, body } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    condition,
                    local_names,
                    function,
                    bindings,
                    instantiations,
                );
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    instantiations,
                );
            }
            ql_ast::StmtKind::Loop { body } => {
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    instantiations,
                );
            }
            ql_ast::StmtKind::For { iterable, body, .. } => {
                collect_dependency_generic_function_instantiations_from_expr(
                    iterable,
                    local_names,
                    function,
                    bindings,
                    instantiations,
                );
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
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
            bindings,
            instantiations,
        );
    }
}

fn collect_dependency_generic_function_instantiations_from_expr(
    expr: &Expr,
    local_names: &BTreeSet<String>,
    function: &FunctionDecl,
    bindings: &TypeBindings,
    instantiations: &mut BTreeSet<TypeBindings>,
) {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::Name(name) = &callee.kind
                && local_names.contains(name)
                && let Some(substitutions) =
                    infer_dependency_generic_function_substitutions(function, args, bindings)
            {
                instantiations.insert(substitutions);
            }
            collect_dependency_generic_function_instantiations_from_expr(
                callee,
                local_names,
                function,
                bindings,
                instantiations,
            );
            for arg in args {
                match arg {
                    CallArg::Positional(value) | CallArg::Named { value, .. } => {
                        collect_dependency_generic_function_instantiations_from_expr(
                            value,
                            local_names,
                            function,
                            bindings,
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
                    bindings,
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
                        bindings,
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
                bindings,
                instantiations,
            );
            collect_dependency_generic_function_instantiations_from_expr(
                right,
                local_names,
                function,
                bindings,
                instantiations,
            );
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_generic_function_instantiations_from_expr(
                expr,
                local_names,
                function,
                bindings,
                instantiations,
            );
        }
        ExprKind::Member { object, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                object,
                local_names,
                function,
                bindings,
                instantiations,
            );
        }
        ExprKind::Bracket { target, items } => {
            collect_dependency_generic_function_instantiations_from_expr(
                target,
                local_names,
                function,
                bindings,
                instantiations,
            );
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    local_names,
                    function,
                    bindings,
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
                bindings,
                instantiations,
            );
            let mut then_bindings = bindings.clone();
            collect_dependency_generic_function_instantiations_from_block(
                then_branch,
                local_names,
                function,
                &mut then_bindings,
                instantiations,
            );
            if let Some(else_branch) = else_branch {
                collect_dependency_generic_function_instantiations_from_expr(
                    else_branch,
                    local_names,
                    function,
                    bindings,
                    instantiations,
                );
            }
        }
        ExprKind::Match { value, arms } => {
            collect_dependency_generic_function_instantiations_from_expr(
                value,
                local_names,
                function,
                bindings,
                instantiations,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_dependency_generic_function_instantiations_from_expr(
                        guard,
                        local_names,
                        function,
                        bindings,
                        instantiations,
                    );
                }
                collect_dependency_generic_function_instantiations_from_expr(
                    &arm.body,
                    local_names,
                    function,
                    bindings,
                    instantiations,
                );
            }
        }
        ExprKind::Closure { body, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                body,
                local_names,
                function,
                bindings,
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
    bindings: &TypeBindings,
) -> Option<TypeBindings> {
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
    let mut substitutions = TypeBindings::new();
    for (param_ty, arg) in regular_params.into_iter().zip(args) {
        let Some(generic_name) = generic_param_name_for_type_expr(param_ty, &generic_names) else {
            continue;
        };
        let arg_ty = infer_dependency_generic_arg_type(arg, bindings)?;
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

fn collect_function_param_type_bindings(function: &FunctionDecl, bindings: &mut TypeBindings) {
    for param in &function.params {
        let Param::Regular { name, ty, .. } = param else {
            continue;
        };
        if let Some(ty) = primitive_type_name_for_type_expr(ty) {
            bindings.insert(name.clone(), ty);
        }
    }
}

fn record_let_type_bindings(
    pattern: &Pattern,
    ty: Option<&TypeExpr>,
    value: &Expr,
    bindings: &mut TypeBindings,
) {
    if let Some(ty) = ty {
        record_pattern_type_bindings(pattern, ty, bindings);
        return;
    }
    if let PatternKind::Name(name) = &pattern.kind
        && let Some(ty) = infer_dependency_generic_expr_type(value, bindings)
    {
        bindings.insert(name.clone(), ty);
    }
}

fn record_pattern_type_bindings(pattern: &Pattern, ty: &TypeExpr, bindings: &mut TypeBindings) {
    match (&pattern.kind, &ty.kind) {
        (PatternKind::Name(name), _) => {
            if let Some(ty) = primitive_type_name_for_type_expr(ty) {
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

fn infer_dependency_generic_arg_type(arg: &CallArg, bindings: &TypeBindings) -> Option<String> {
    let expr = match arg {
        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => expr,
    };
    infer_dependency_generic_expr_type(expr, bindings)
}

fn infer_dependency_generic_expr_type(expr: &Expr, bindings: &TypeBindings) -> Option<String> {
    match &expr.kind {
        ExprKind::Integer(_) => Some("Int".to_owned()),
        ExprKind::Bool(_) => Some("Bool".to_owned()),
        ExprKind::String { .. } => Some("String".to_owned()),
        ExprKind::Name(name) => bindings.get(name).cloned(),
        _ => None,
    }
}

fn primitive_type_name_for_type_expr(ty: &TypeExpr) -> Option<String> {
    let TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let [name] = path.segments.as_slice() else {
        return None;
    };
    matches!(name.as_str(), "Int" | "Bool" | "String").then(|| name.clone())
}
