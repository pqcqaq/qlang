use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{CallArg, Expr, ExprKind, GlobalDecl, ItemKind, Module, Visibility};

#[derive(Clone, Copy)]
pub(crate) struct DependencyPublicGlobalBridgeCandidate<'a> {
    pub(crate) item: &'a ql_ast::Item,
    pub(crate) global: &'a GlobalDecl,
}

#[derive(Default)]
pub(crate) struct DependencyPublicGlobalExprDependencies {
    pub(crate) globals: BTreeSet<String>,
    pub(crate) functions: BTreeSet<String>,
}

pub(crate) fn dependency_public_global_bridge_candidates<'a>(
    module: &'a Module,
) -> BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>> {
    let mut candidates = BTreeMap::new();
    for item in &module.items {
        let global = match &item.kind {
            ItemKind::Const(global) | ItemKind::Static(global)
                if global.visibility == Visibility::Public =>
            {
                global
            }
            _ => continue,
        };
        candidates.insert(
            global.name.clone(),
            DependencyPublicGlobalBridgeCandidate { item, global },
        );
    }
    candidates
}

pub(crate) fn dependency_public_global_dependencies<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
) -> Option<DependencyPublicGlobalExprDependencies> {
    let candidate = global_candidates.get(symbol_name)?;
    let mut dependencies = DependencyPublicGlobalExprDependencies::default();
    collect_dependency_public_global_expr_dependencies(
        &candidate.global.value,
        global_candidates,
        function_candidates,
        &mut dependencies,
    )
    .then_some(dependencies)
}

pub(crate) fn dependency_public_global_bridge_order<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    if collect_dependency_public_global_bridge_order(
        symbol_name,
        global_candidates,
        function_candidates,
        &mut visiting,
        &mut visited,
        &mut ordered,
    ) {
        Some(ordered)
    } else {
        None
    }
}

fn collect_dependency_public_global_bridge_order<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> bool {
    if visited.contains(symbol_name) {
        return true;
    }
    let Some(_) = global_candidates.get(symbol_name) else {
        return false;
    };
    if !visiting.insert(symbol_name.to_owned()) {
        return false;
    }

    let Some(dependencies) =
        dependency_public_global_dependencies(symbol_name, global_candidates, function_candidates)
    else {
        visiting.remove(symbol_name);
        return false;
    };

    for dependency in dependencies.globals {
        if !collect_dependency_public_global_bridge_order(
            &dependency,
            global_candidates,
            function_candidates,
            visiting,
            visited,
            ordered,
        ) {
            visiting.remove(symbol_name);
            return false;
        }
    }

    visiting.remove(symbol_name);
    visited.insert(symbol_name.to_owned());
    ordered.push(symbol_name.to_owned());
    true
}

fn collect_dependency_public_global_expr_dependencies<'a>(
    expr: &Expr,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
    dependencies: &mut DependencyPublicGlobalExprDependencies,
) -> bool {
    match &expr.kind {
        ExprKind::Integer(_) | ExprKind::String { .. } | ExprKind::Bool(_) => true,
        ExprKind::Name(name) => {
            if global_candidates.contains_key(name) {
                dependencies.globals.insert(name.clone());
                true
            } else if function_candidates.contains(name) {
                dependencies.functions.insert(name.clone());
                true
            } else {
                false
            }
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => items.iter().all(|item| {
            collect_dependency_public_global_expr_dependencies(
                item,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }),
        ExprKind::RepeatArray { value, .. } => collect_dependency_public_global_expr_dependencies(
            value,
            global_candidates,
            function_candidates,
            dependencies,
        ),
        ExprKind::StructLiteral { fields, .. } => fields.iter().all(|field| {
            field.value.as_ref().is_some_and(|value| {
                collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                )
            })
        }),
        ExprKind::Binary { left, right, .. } => {
            collect_dependency_public_global_expr_dependencies(
                left,
                global_candidates,
                function_candidates,
                dependencies,
            ) && collect_dependency_public_global_expr_dependencies(
                right,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_public_global_expr_dependencies(
                expr,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }
        ExprKind::Call { callee, args } => {
            collect_dependency_public_global_expr_dependencies(
                callee,
                global_candidates,
                function_candidates,
                dependencies,
            ) && args.iter().all(|arg| match arg {
                CallArg::Positional(value) => collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                ),
                CallArg::Named { value, .. } => collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                ),
            })
        }
        ExprKind::Member { object, .. } => collect_dependency_public_global_expr_dependencies(
            object,
            global_candidates,
            function_candidates,
            dependencies,
        ),
        ExprKind::Bracket { target, items } => {
            collect_dependency_public_global_expr_dependencies(
                target,
                global_candidates,
                function_candidates,
                dependencies,
            ) && items.iter().all(|item| {
                collect_dependency_public_global_expr_dependencies(
                    item,
                    global_candidates,
                    function_candidates,
                    dependencies,
                )
            })
        }
        ExprKind::NoneLiteral
        | ExprKind::Block(_)
        | ExprKind::Unsafe(_)
        | ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::Closure { .. } => false,
    }
}
