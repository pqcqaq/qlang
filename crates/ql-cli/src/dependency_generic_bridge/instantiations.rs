use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{
    self, BinaryOp, CallArg, Expr, ExprKind, FunctionDecl, ItemKind, Module, Param, Pattern,
    PatternKind, TypeExpr, TypeExprKind, UnaryOp,
};
use ql_span::Span;

pub(super) type TypeSubstitutions = BTreeMap<String, String>;
type ValueTypeBindings = BTreeMap<String, InferredType>;
pub(super) type FunctionTypeBindings = BTreeMap<String, FunctionDecl>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiation {
    pub(super) callee_span: Span,
    pub(super) substitutions: TypeSubstitutions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InferredType {
    rendered: String,
    kind: InferredTypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InferredTypeKind {
    Pointer {
        is_const: bool,
        inner: Box<InferredType>,
    },
    Array {
        element: Box<InferredType>,
        len: String,
    },
    Named {
        path: Vec<String>,
        args: Vec<InferredType>,
    },
    Tuple(Vec<InferredType>),
    Callable {
        params: Vec<InferredType>,
        ret: Box<InferredType>,
    },
}

impl InferredType {
    fn primitive(name: &str) -> Self {
        Self {
            rendered: name.to_owned(),
            kind: InferredTypeKind::Named {
                path: vec![name.to_owned()],
                args: Vec::new(),
            },
        }
    }

    fn from_type_expr(ty: &TypeExpr) -> Option<Self> {
        match &ty.kind {
            TypeExprKind::Named { path, args } => {
                let args = args
                    .iter()
                    .map(Self::from_type_expr)
                    .collect::<Option<Vec<_>>>()?;
                let mut rendered = path.segments.join(".");
                if !args.is_empty() {
                    rendered.push('[');
                    rendered.push_str(
                        &args
                            .iter()
                            .map(|arg| arg.rendered.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    rendered.push(']');
                }
                Some(Self {
                    rendered,
                    kind: InferredTypeKind::Named {
                        path: path.segments.clone(),
                        args,
                    },
                })
            }
            TypeExprKind::Tuple(items) => {
                let items = items
                    .iter()
                    .map(Self::from_type_expr)
                    .collect::<Option<Vec<_>>>()?;
                let mut rendered = String::from("(");
                rendered.push_str(
                    &items
                        .iter()
                        .map(|item| item.rendered.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                if items.len() == 1 {
                    rendered.push(',');
                }
                rendered.push(')');
                Some(Self {
                    rendered,
                    kind: InferredTypeKind::Tuple(items),
                })
            }
            TypeExprKind::Array { element, len } => {
                let element = Self::from_type_expr(element)?;
                Some(Self {
                    rendered: format!("[{}; {len}]", element.rendered),
                    kind: InferredTypeKind::Array {
                        element: Box::new(element),
                        len: len.clone(),
                    },
                })
            }
            TypeExprKind::Pointer { is_const, inner } => {
                let inner = Self::from_type_expr(inner)?;
                let qualifier = if *is_const { "const " } else { "" };
                Some(Self {
                    rendered: format!("*{}{}", qualifier, inner.rendered),
                    kind: InferredTypeKind::Pointer {
                        is_const: *is_const,
                        inner: Box::new(inner),
                    },
                })
            }
            TypeExprKind::Callable { params, ret } => {
                let params = params
                    .iter()
                    .map(Self::from_type_expr)
                    .collect::<Option<Vec<_>>>()?;
                let ret = Self::from_type_expr(ret)?;
                Some(Self {
                    rendered: format!(
                        "({}) -> {}",
                        params
                            .iter()
                            .map(|param| param.rendered.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        ret.rendered
                    ),
                    kind: InferredTypeKind::Callable {
                        params,
                        ret: Box::new(ret),
                    },
                })
            }
        }
    }
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

pub(super) fn collect_public_function_call_instantiations(
    root_module: &Module,
    module_import_path: &[String],
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let local_names =
        dependency_imported_local_names(root_module, module_import_path, function.name.as_str());
    if local_names.is_empty() {
        return Vec::new();
    }

    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut instantiations = Vec::new();
    for item in &root_module.items {
        collect_dependency_generic_function_instantiations_from_item(
            item,
            &local_names,
            function,
            &root_bindings,
            function_bindings,
            &mut instantiations,
        );
    }
    instantiations
}

pub(super) fn collect_local_function_call_instantiations(
    root_module: &Module,
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let local_names = BTreeSet::from([function.name.clone()]);
    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut instantiations = Vec::new();
    for item in &root_module.items {
        let ItemKind::Function(root_function) = &item.kind else {
            collect_dependency_generic_function_instantiations_from_item(
                item,
                &local_names,
                function,
                &root_bindings,
                function_bindings,
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
    root_bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
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
                    instantiations,
                );
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    function_bindings,
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
                    instantiations,
                );
                let mut body_bindings = bindings.clone();
                collect_dependency_generic_function_instantiations_from_block(
                    body,
                    local_names,
                    function,
                    &mut body_bindings,
                    function_bindings,
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
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
) {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::Name(name) = &callee.kind
                && local_names.contains(name)
                && let Some(substitutions) = infer_dependency_generic_function_substitutions(
                    function,
                    args,
                    expected_ty,
                    bindings,
                    function_bindings,
                )
            {
                instantiations.push(PublicFunctionCallInstantiation {
                    callee_span: callee.span,
                    substitutions,
                });
            }
            collect_dependency_generic_function_instantiations_from_expr(
                callee,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                instantiations,
            );
            for arg in args {
                match arg {
                    CallArg::Positional(value) | CallArg::Named { value, .. } => {
                        collect_dependency_generic_function_instantiations_from_expr(
                            value,
                            None,
                            return_expected_ty,
                            local_names,
                            function,
                            bindings,
                            function_bindings,
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
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    instantiations,
                );
            }
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
                instantiations,
            );
            let mut then_bindings = bindings.clone();
            collect_dependency_generic_function_instantiations_from_block(
                then_branch,
                local_names,
                function,
                &mut then_bindings,
                function_bindings,
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

fn generic_param_name_for_type_expr<'a>(
    ty: &TypeExpr,
    generic_names: &BTreeSet<&'a str>,
) -> Option<&'a str> {
    let TypeExprKind::Named { path, args } = &ty.kind else {
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

fn type_expr_mentions_generic(ty: &TypeExpr, generic_names: &BTreeSet<&str>) -> bool {
    if generic_param_name_for_type_expr(ty, generic_names).is_some() {
        return true;
    }
    match &ty.kind {
        TypeExprKind::Pointer { inner, .. } => type_expr_mentions_generic(inner, generic_names),
        TypeExprKind::Array { element, len } => {
            generic_names.contains(len.as_str())
                || type_expr_mentions_generic(element, generic_names)
        }
        TypeExprKind::Named { args, .. } | TypeExprKind::Tuple(args) => args
            .iter()
            .any(|arg| type_expr_mentions_generic(arg, generic_names)),
        TypeExprKind::Callable { params, ret } => {
            params
                .iter()
                .any(|param| type_expr_mentions_generic(param, generic_names))
                || type_expr_mentions_generic(ret, generic_names)
        }
    }
}

fn collect_generic_type_substitutions(
    param_ty: &TypeExpr,
    arg_ty: &InferredType,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    if let Some(generic_name) = generic_param_name_for_type_expr(param_ty, generic_names) {
        return bind_generic_type_substitution(generic_name, arg_ty, substitutions);
    }

    match (&param_ty.kind, &arg_ty.kind) {
        (
            TypeExprKind::Named { path, args },
            InferredTypeKind::Named {
                path: arg_path,
                args: arg_args,
            },
        ) => {
            path.segments == *arg_path
                && args.len() == arg_args.len()
                && args.iter().zip(arg_args).all(|(param_arg, arg_arg)| {
                    collect_generic_type_substitutions(
                        param_arg,
                        arg_arg,
                        generic_names,
                        substitutions,
                    )
                })
        }
        (TypeExprKind::Tuple(params), InferredTypeKind::Tuple(args)) => {
            params.len() == args.len()
                && params.iter().zip(args).all(|(param_item, arg_item)| {
                    collect_generic_type_substitutions(
                        param_item,
                        arg_item,
                        generic_names,
                        substitutions,
                    )
                })
        }
        (
            TypeExprKind::Array {
                element: param_element,
                len: param_len,
            },
            InferredTypeKind::Array {
                element: arg_element,
                len: arg_len,
            },
        ) => {
            bind_generic_len_substitution(param_len, arg_len, generic_names, substitutions)
                && collect_generic_type_substitutions(
                    param_element,
                    arg_element,
                    generic_names,
                    substitutions,
                )
        }
        (
            TypeExprKind::Pointer {
                is_const: param_const,
                inner: param_inner,
            },
            InferredTypeKind::Pointer {
                is_const: arg_const,
                inner: arg_inner,
            },
        ) => {
            param_const == arg_const
                && collect_generic_type_substitutions(
                    param_inner,
                    arg_inner,
                    generic_names,
                    substitutions,
                )
        }
        (
            TypeExprKind::Callable {
                params: param_params,
                ret: param_ret,
            },
            InferredTypeKind::Callable {
                params: arg_params,
                ret: arg_ret,
            },
        ) => {
            param_params.len() == arg_params.len()
                && param_params
                    .iter()
                    .zip(arg_params)
                    .all(|(param_param, arg_param)| {
                        collect_generic_type_substitutions(
                            param_param,
                            arg_param,
                            generic_names,
                            substitutions,
                        )
                    })
                && collect_generic_type_substitutions(
                    param_ret,
                    arg_ret,
                    generic_names,
                    substitutions,
                )
        }
        _ => false,
    }
}

fn bind_generic_len_substitution(
    param_len: &str,
    arg_len: &str,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    if generic_names.contains(param_len) {
        return bind_generic_rendered_substitution(param_len, arg_len, substitutions);
    }
    param_len == arg_len
}

fn bind_generic_type_substitution(
    generic_name: &str,
    arg_ty: &InferredType,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    bind_generic_rendered_substitution(generic_name, &arg_ty.rendered, substitutions)
}

fn bind_generic_rendered_substitution(
    generic_name: &str,
    rendered: &str,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    match substitutions.get(generic_name) {
        Some(existing) => existing == rendered,
        None => {
            substitutions.insert(generic_name.to_owned(), rendered.to_owned());
            true
        }
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

fn inferred_type_from_type_expr_with_substitutions(
    ty: &TypeExpr,
    substitutions: &TypeSubstitutions,
) -> Option<InferredType> {
    match &ty.kind {
        TypeExprKind::Named { path, args } => {
            if args.is_empty()
                && let [name] = path.segments.as_slice()
                && let Some(substitution) = substitutions.get(name)
            {
                return Some(inferred_type_from_rendered_substitution(substitution));
            }

            let args = args
                .iter()
                .map(|arg| inferred_type_from_type_expr_with_substitutions(arg, substitutions))
                .collect::<Option<Vec<_>>>()?;
            let mut rendered = path.segments.join(".");
            if !args.is_empty() {
                rendered.push('[');
                rendered.push_str(
                    &args
                        .iter()
                        .map(|arg| arg.rendered.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                rendered.push(']');
            }
            Some(InferredType {
                rendered,
                kind: InferredTypeKind::Named {
                    path: path.segments.clone(),
                    args,
                },
            })
        }
        TypeExprKind::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| inferred_type_from_type_expr_with_substitutions(item, substitutions))
                .collect::<Option<Vec<_>>>()?;
            Some(InferredType {
                rendered: render_inferred_tuple_type(&items),
                kind: InferredTypeKind::Tuple(items),
            })
        }
        TypeExprKind::Array { element, len } => {
            let element = inferred_type_from_type_expr_with_substitutions(element, substitutions)?;
            let len = substitutions
                .get(len)
                .cloned()
                .unwrap_or_else(|| len.clone());
            Some(InferredType {
                rendered: format!("[{}; {len}]", element.rendered),
                kind: InferredTypeKind::Array {
                    element: Box::new(element),
                    len,
                },
            })
        }
        TypeExprKind::Pointer { is_const, inner } => {
            let inner = inferred_type_from_type_expr_with_substitutions(inner, substitutions)?;
            let qualifier = if *is_const { "const " } else { "" };
            Some(InferredType {
                rendered: format!("*{}{}", qualifier, inner.rendered),
                kind: InferredTypeKind::Pointer {
                    is_const: *is_const,
                    inner: Box::new(inner),
                },
            })
        }
        TypeExprKind::Callable { params, ret } => {
            let params = params
                .iter()
                .map(|param| inferred_type_from_type_expr_with_substitutions(param, substitutions))
                .collect::<Option<Vec<_>>>()?;
            let ret = inferred_type_from_type_expr_with_substitutions(ret, substitutions)?;
            Some(InferredType {
                rendered: format!(
                    "({}) -> {}",
                    params
                        .iter()
                        .map(|param| param.rendered.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    ret.rendered
                ),
                kind: InferredTypeKind::Callable {
                    params,
                    ret: Box::new(ret),
                },
            })
        }
    }
}

fn inferred_type_from_rendered_substitution(rendered: &str) -> InferredType {
    match rendered {
        "Int" | "Bool" | "String" => InferredType::primitive(rendered),
        _ => InferredType {
            rendered: rendered.to_owned(),
            kind: InferredTypeKind::Named {
                path: rendered.split('.').map(str::to_owned).collect(),
                args: Vec::new(),
            },
        },
    }
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

fn render_inferred_tuple_type(items: &[InferredType]) -> String {
    let mut rendered = String::from("(");
    rendered.push_str(
        &items
            .iter()
            .map(|item| item.rendered.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );
    if items.len() == 1 {
        rendered.push(',');
    }
    rendered.push(')');
    rendered
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

fn are_inferred_bool_types(left: &InferredType, right: &InferredType) -> bool {
    is_inferred_bool_type(left) && is_inferred_bool_type(right)
}

fn is_inferred_bool_type(ty: &InferredType) -> bool {
    is_inferred_builtin_type(ty, "Bool")
}

fn is_inferred_equality_comparable_type(ty: &InferredType) -> bool {
    is_inferred_numeric_type(ty) || is_inferred_bool_type(ty) || is_inferred_string_type(ty)
}

fn is_inferred_ordered_comparable_type(ty: &InferredType) -> bool {
    is_inferred_numeric_type(ty) || is_inferred_string_type(ty)
}

fn is_inferred_string_type(ty: &InferredType) -> bool {
    is_inferred_builtin_type(ty, "String")
}

fn is_inferred_numeric_type(ty: &InferredType) -> bool {
    matches!(
        inferred_named_builtin_type(ty),
        Some(
            "Int"
                | "UInt"
                | "I8"
                | "I16"
                | "I32"
                | "I64"
                | "ISize"
                | "U8"
                | "U16"
                | "U32"
                | "U64"
                | "USize"
                | "F32"
                | "F64"
        )
    )
}

fn is_inferred_builtin_type(ty: &InferredType, expected: &str) -> bool {
    inferred_named_builtin_type(ty) == Some(expected)
}

fn inferred_named_builtin_type(ty: &InferredType) -> Option<&str> {
    let InferredTypeKind::Named { path, args } = &ty.kind else {
        return None;
    };
    if !args.is_empty() || path.len() != 1 {
        return None;
    }
    path.first().map(String::as_str)
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
    fn infers_substitution_from_later_argument_when_nested_call_arg_is_untyped() {
        let dependency = parse_module(
            r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn unwrap_or[T](value: Option[T], fallback: T) -> T {
    return fallback
}
"#,
        );
        let root = parse_module(
            r#"
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or

fn run() -> Int {
    return option_unwrap_or(option_some(42), 0)
}
"#,
        );

        let instantiations = collect_public_function_instantiations(
            &root,
            &["std".to_owned(), "option".to_owned()],
            function(&dependency, "unwrap_or"),
        );

        assert_eq!(instantiations.len(), 1);
        let substitutions = instantiations
            .iter()
            .next()
            .expect("one substitution should be inferred");
        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    }

    #[test]
    fn infers_substitution_from_named_arguments() {
        let dependency = parse_module(
            r#"
package dep

pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
        );
        let root = parse_module(
            r#"
use dep.choose as choose

fn run() -> Int {
    return choose(value: 42, fallback: 0)
}
"#,
        );

        let instantiations = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "choose"),
        );

        assert_eq!(instantiations.len(), 1);
        let substitutions = instantiations
            .iter()
            .next()
            .expect("one named-argument substitution should be inferred");
        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    }

    #[test]
    fn infers_substitutions_from_scalar_expressions() {
        let dependency = parse_module(
            r#"
package dep

pub fn identity[T](value: T) -> T {
    return value
}
"#,
        );
        let root = parse_module(
            r#"
use dep.identity as identity

fn run() -> Int {
    let number: Int = identity(1 + 2)
    let flag: Bool = identity(!(false || false))
    let ordered: Bool = identity(1 < 2)
    if flag && ordered {
        return number
    }
    return 0
}
"#,
        );

        let substitutions = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "identity"),
        );

        assert_eq!(substitutions.len(), 2);
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("Int") })
        );
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("Bool") })
        );
    }

    #[test]
    fn infers_substitutions_from_tuple_and_array_literals() {
        let dependency = parse_module(
            r#"
package dep

pub fn identity[T](value: T) -> T {
    return value
}
"#,
        );
        let root = parse_module(
            r#"
use dep.identity as identity

fn run() -> Int {
    let pair: (Int, Bool) = identity((1 + 2, !false))
    let values: [Int; 3] = identity([1, 2 + 3, 4])
    if pair[1] {
        return values[0]
    }
    return 0
}
"#,
        );

        let substitutions = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "identity"),
        );

        assert_eq!(substitutions.len(), 2);
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("(Int, Bool)") })
        );
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("[Int; 3]") })
        );
    }

    #[test]
    fn infers_substitutions_from_projection_and_control_flow_expressions() {
        let dependency = parse_module(
            r#"
package dep

pub fn identity[T](value: T) -> T {
    return value
}
"#,
        );
        let root = parse_module(
            r#"
use dep.identity as identity

fn run(flag: Bool) -> Int {
    let values: [Int; 3] = [1, 2, 3]
    let pair: (Int, Bool) = (5, flag)
    let picked: Int = identity(values[1])
    let tuple_flag: Bool = identity(pair[1])
    let selected: Int = identity(if tuple_flag { picked } else { 0 })
    let matched: Bool = identity(match selected {
        0 => false,
        _ => tuple_flag,
    })
    if matched {
        return selected
    }
    return 0
}
"#,
        );

        let substitutions = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "identity"),
        );

        assert_eq!(substitutions.len(), 2);
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("Int") })
        );
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("Bool") })
        );
    }

    #[test]
    fn infers_substitutions_from_generic_array_parameters() {
        let dependency = parse_module(
            r#"
package dep

pub fn first3[T](values: [T; 3]) -> T {
    return values[0]
}
"#,
        );
        let root = parse_module(
            r#"
use dep.first3 as first3

fn run() -> Int {
    let value: Int = first3([1, 2 + 3, 4])
    let flag: Bool = first3([true, false, true])
    if flag {
        return value
    }
    return 0
}
"#,
        );

        let substitutions = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "first3"),
        );

        assert_eq!(substitutions.len(), 2);
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("Int") })
        );
        assert!(
            substitutions
                .iter()
                .any(|item| { item.get("T").map(String::as_str) == Some("Bool") })
        );
    }

    #[test]
    fn infers_substitutions_from_generic_array_length_parameters() {
        let dependency = parse_module(
            r#"
package dep

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        );
        let root = parse_module(
            r#"
use dep.first as first

fn run() -> Int {
    let value: Int = first([1, 2 + 3, 4])
    let flag: Bool = first([true, false, true, false])
    if flag {
        return value
    }
    return 0
}
"#,
        );

        let substitutions = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "first"),
        );

        assert_eq!(substitutions.len(), 2);
        assert!(substitutions.iter().any(|item| {
            item.get("T").map(String::as_str) == Some("Int")
                && item.get("N").map(String::as_str) == Some("3")
        }));
        assert!(substitutions.iter().any(|item| {
            item.get("T").map(String::as_str) == Some("Bool")
                && item.get("N").map(String::as_str) == Some("4")
        }));
    }

    #[test]
    fn infers_array_substitution_from_partially_typed_array_literal_items() {
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

fn hidden_value() -> Int {
    return 2
}

fn run() -> Int {
    let values: [Int; 3] = reverse([1, hidden_value(), 3])
    return values[0]
}
"#,
        );

        let substitutions = collect_public_function_instantiations(
            &root,
            &["dep".to_owned()],
            function(&dependency, "reverse"),
        );

        assert_eq!(substitutions.len(), 1);
        let substitutions = substitutions
            .iter()
            .next()
            .expect("one array literal substitution should be inferred");
        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
        assert_eq!(substitutions.get("N").map(String::as_str), Some("3"));
    }

    #[test]
    fn infers_substitution_from_single_field_generic_variant_call() {
        let dependency = parse_module(
            r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn is_some[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => true,
        Option.None => false,
    }
}
"#,
        );
        let root = parse_module(
            r#"
use std.option.Option as Option
use std.option.is_some as option_is_some

fn run() -> Int {
    if option_is_some(Option.Some(42)) {
        return 0
    }
    return 1
}
"#,
        );

        let instantiations = collect_public_function_instantiations(
            &root,
            &["std".to_owned(), "option".to_owned()],
            function(&dependency, "is_some"),
        );

        assert_eq!(instantiations.len(), 1);
        let substitutions = instantiations
            .iter()
            .next()
            .expect("one substitution should be inferred");
        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    }

    #[test]
    fn infers_substitution_from_explicit_result_context() {
        let dependency = parse_module(
            r#"
package std.result

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn ok[T, E](value: T) -> Result[T, E] {
    return Result.Ok(value)
}

pub fn err[T, E](error: E) -> Result[T, E] {
    return Result.Err(error)
}
"#,
        );
        let root = parse_module(
            r#"
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.err as result_err

fn make_ok() -> Result[Int, Int] {
    return result_ok(42)
}

fn run() -> Int {
    let failed: Result[Int, Int] = result_err(3)
    return 0
}
"#,
        );

        let ok_instantiations = collect_public_function_instantiations(
            &root,
            &["std".to_owned(), "result".to_owned()],
            function(&dependency, "ok"),
        );
        let err_instantiations = collect_public_function_instantiations(
            &root,
            &["std".to_owned(), "result".to_owned()],
            function(&dependency, "err"),
        );

        assert_eq!(ok_instantiations.len(), 1);
        let ok = ok_instantiations
            .iter()
            .next()
            .expect("ok should infer one substitution");
        assert_eq!(ok.get("T").map(String::as_str), Some("Int"));
        assert_eq!(ok.get("E").map(String::as_str), Some("Int"));

        assert_eq!(err_instantiations.len(), 1);
        let err = err_instantiations
            .iter()
            .next()
            .expect("err should infer one substitution");
        assert_eq!(err.get("T").map(String::as_str), Some("Int"));
        assert_eq!(err.get("E").map(String::as_str), Some("Int"));
    }

    #[test]
    fn infers_zero_argument_substitution_from_explicit_option_context() {
        let dependency = parse_module(
            r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
}
"#,
        );
        let root = parse_module(
            r#"
use std.option.Option as Option
use std.option.none_option as option_none

fn make_none() -> Option[Int] {
    return option_none()
}

fn run() -> Int {
    let value: Option[Int] = option_none()
    return 0
}
"#,
        );

        let instantiations = collect_public_function_instantiations(
            &root,
            &["std".to_owned(), "option".to_owned()],
            function(&dependency, "none_option"),
        );

        assert_eq!(instantiations.len(), 1);
        let substitutions = instantiations
            .iter()
            .next()
            .expect("none_option should infer one substitution");
        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    }
}
