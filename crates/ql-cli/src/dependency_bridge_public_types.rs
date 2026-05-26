use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{FunctionDecl, ItemKind, Module, Param, Visibility};

use crate::dependency_bridge_names::supports_dependency_public_method_import_bridge;

#[derive(Clone, Copy)]
pub(crate) struct DependencyPublicTypeBridgeCandidate<'a> {
    pub(crate) item: &'a ql_ast::Item,
    pub(crate) decl: DependencyPublicTypeDecl<'a>,
}

#[derive(Clone, Copy)]
pub(crate) enum DependencyPublicTypeDecl<'a> {
    Struct(&'a ql_ast::StructDecl),
    Enum(&'a ql_ast::EnumDecl),
    TypeAlias(&'a ql_ast::TypeAliasDecl),
}

impl<'a> DependencyPublicTypeDecl<'a> {
    fn name(self) -> &'a str {
        match self {
            Self::Struct(struct_decl) => struct_decl.name.as_str(),
            Self::Enum(enum_decl) => enum_decl.name.as_str(),
            Self::TypeAlias(alias) => alias.name.as_str(),
        }
    }

    pub(crate) fn is_generic(self) -> bool {
        match self {
            Self::Struct(struct_decl) => !struct_decl.generics.is_empty(),
            Self::Enum(enum_decl) => !enum_decl.generics.is_empty(),
            Self::TypeAlias(alias) => !alias.generics.is_empty(),
        }
    }
}

pub(crate) fn dependency_public_type_bridge_candidates<'a>(
    module: &'a Module,
) -> BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>> {
    let mut candidates = BTreeMap::new();
    for item in &module.items {
        let decl = match &item.kind {
            ItemKind::Struct(struct_decl) if struct_decl.visibility == Visibility::Public => {
                DependencyPublicTypeDecl::Struct(struct_decl)
            }
            ItemKind::Enum(enum_decl) if enum_decl.visibility == Visibility::Public => {
                DependencyPublicTypeDecl::Enum(enum_decl)
            }
            ItemKind::TypeAlias(alias)
                if alias.visibility == Visibility::Public
                    && !alias.is_opaque
                    && alias.generics.is_empty() =>
            {
                DependencyPublicTypeDecl::TypeAlias(alias)
            }
            _ => continue,
        };
        candidates.insert(
            decl.name().to_owned(),
            DependencyPublicTypeBridgeCandidate { item, decl },
        );
    }
    candidates
}

pub(crate) fn dependency_public_struct_method_bridge_candidates<'a>(
    module: &'a Module,
    struct_name: &str,
) -> BTreeMap<String, &'a FunctionDecl> {
    let mut impl_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();
    let mut extend_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();
    let mut trait_impl_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();

    for item in &module.items {
        match &item.kind {
            ItemKind::Impl(impl_block)
                if impl_block.trait_ty.is_none()
                    && dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
            {
                for method in impl_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    impl_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            ItemKind::Impl(impl_block)
                if impl_block.trait_ty.is_some()
                    && dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
            {
                for method in impl_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    trait_impl_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            ItemKind::Extend(extend_block)
                if dependency_type_expr_targets_struct(&extend_block.target, struct_name) =>
            {
                for method in extend_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    extend_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            _ => {}
        }
    }

    let mut methods = BTreeMap::new();
    for (name, candidates) in impl_candidates {
        if candidates.len() == 1 {
            methods.insert(name, candidates.into_iter().next().unwrap());
        }
    }
    for (name, candidates) in extend_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    for (name, candidates) in trait_impl_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    methods
}

pub(crate) fn collect_dependency_public_type_expr_dependencies<'a>(
    ty: &ql_ast::TypeExpr,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
    dependencies: &mut BTreeSet<String>,
) {
    match &ty.kind {
        ql_ast::TypeExprKind::Pointer { inner, .. } => {
            collect_dependency_public_type_expr_dependencies(inner, type_candidates, dependencies);
        }
        ql_ast::TypeExprKind::Array { element, .. } => {
            collect_dependency_public_type_expr_dependencies(
                element,
                type_candidates,
                dependencies,
            );
        }
        ql_ast::TypeExprKind::Named { path, args } => {
            if let [name] = path.segments.as_slice() {
                if type_candidates.contains_key(name) {
                    dependencies.insert(name.clone());
                }
            }
            for arg in args {
                collect_dependency_public_type_expr_dependencies(
                    arg,
                    type_candidates,
                    dependencies,
                );
            }
        }
        ql_ast::TypeExprKind::Tuple(items) => {
            for item in items {
                collect_dependency_public_type_expr_dependencies(
                    item,
                    type_candidates,
                    dependencies,
                );
            }
        }
        ql_ast::TypeExprKind::Callable { params, ret } => {
            for param in params {
                collect_dependency_public_type_expr_dependencies(
                    param,
                    type_candidates,
                    dependencies,
                );
            }
            collect_dependency_public_type_expr_dependencies(ret, type_candidates, dependencies);
        }
    }
}

pub(crate) fn dependency_public_type_bridge_order<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    if collect_dependency_public_type_bridge_order(
        symbol_name,
        type_candidates,
        &mut visiting,
        &mut visited,
        &mut ordered,
    ) {
        Some(ordered)
    } else {
        None
    }
}

pub(crate) fn collect_dependency_public_function_type_dependencies<'a>(
    function: &FunctionDecl,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();
    for param in &function.params {
        if let Param::Regular { ty, .. } = param {
            collect_dependency_public_type_expr_dependencies(
                ty,
                type_candidates,
                &mut dependencies,
            );
        }
    }
    if let Some(return_type) = &function.return_type {
        collect_dependency_public_type_expr_dependencies(
            return_type,
            type_candidates,
            &mut dependencies,
        );
    }
    dependencies
}

fn dependency_type_expr_targets_struct(ty: &ql_ast::TypeExpr, struct_name: &str) -> bool {
    let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| segment == struct_name)
}

fn dependency_public_type_dependencies<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> Option<BTreeSet<String>> {
    let candidate = type_candidates.get(symbol_name)?;
    let mut dependencies = BTreeSet::new();
    match candidate.decl {
        DependencyPublicTypeDecl::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                collect_dependency_public_type_expr_dependencies(
                    &field.ty,
                    type_candidates,
                    &mut dependencies,
                );
            }
        }
        DependencyPublicTypeDecl::Enum(enum_decl) => {
            for variant in &enum_decl.variants {
                match &variant.fields {
                    ql_ast::VariantFields::Unit => {}
                    ql_ast::VariantFields::Tuple(items) => {
                        for item in items {
                            collect_dependency_public_type_expr_dependencies(
                                item,
                                type_candidates,
                                &mut dependencies,
                            );
                        }
                    }
                    ql_ast::VariantFields::Struct(fields) => {
                        for field in fields {
                            collect_dependency_public_type_expr_dependencies(
                                &field.ty,
                                type_candidates,
                                &mut dependencies,
                            );
                        }
                    }
                }
            }
        }
        DependencyPublicTypeDecl::TypeAlias(alias) => {
            collect_dependency_public_type_expr_dependencies(
                &alias.ty,
                type_candidates,
                &mut dependencies,
            );
        }
    }
    Some(dependencies)
}

fn collect_dependency_public_type_bridge_order<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> bool {
    if visited.contains(symbol_name) {
        return true;
    }
    let Some(_) = type_candidates.get(symbol_name) else {
        return false;
    };
    if !visiting.insert(symbol_name.to_owned()) {
        return false;
    }

    let Some(dependencies) = dependency_public_type_dependencies(symbol_name, type_candidates)
    else {
        visiting.remove(symbol_name);
        return false;
    };

    for dependency in dependencies {
        if !collect_dependency_public_type_bridge_order(
            &dependency,
            type_candidates,
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
