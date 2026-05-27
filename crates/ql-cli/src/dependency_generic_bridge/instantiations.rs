use std::collections::BTreeSet;

use super::function_bindings::{FunctionTypeBindings, dependency_imported_local_names};
pub(super) use super::instantiation_scanner::PublicFunctionCallInstantiation;
use super::instantiation_scanner::{
    collect_dependency_generic_function_instantiations_from_block,
    collect_dependency_generic_function_instantiations_from_item,
};
use super::substitutions::TypeSubstitutions;
use super::value_bindings::{
    ValueTypeBindings, collect_function_param_type_bindings_with_substitutions,
    collect_root_value_type_bindings,
};
use ql_ast::{FunctionDecl, ItemKind, Module};

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

#[cfg(test)]
#[path = "instantiations_tests.rs"]
mod tests;
