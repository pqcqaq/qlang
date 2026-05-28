use std::collections::BTreeSet;

use super::enum_bindings::EnumTypeBindings;
#[cfg(test)]
use super::enum_bindings::collect_root_call_enum_type_bindings;
use super::function_bindings::{FunctionTypeBindings, dependency_imported_local_names};
use super::instantiation_block_scanner::collect_dependency_generic_function_instantiations_from_block;
use super::instantiation_scan_context::InstantiationScanContext;
pub(super) use super::instantiation_scan_context::PublicFunctionCallInstantiation;
use super::instantiation_scanner::collect_dependency_generic_function_instantiations_from_item;
use super::struct_bindings::StructTypeBindings;
#[cfg(test)]
use super::struct_bindings::collect_root_call_struct_type_bindings;
use super::substitutions::TypeSubstitutions;
use super::value_bindings::{
    ValueTypeBindings, collect_function_param_type_bindings_with_substitutions,
    collect_root_value_type_bindings,
};
use ql_ast::{FunctionDecl, Item, ItemKind, Module};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiations {
    pub(super) saw_call: bool,
    pub(super) instantiations: Vec<PublicFunctionCallInstantiation>,
}

impl PublicFunctionCallInstantiations {
    fn none() -> Self {
        Self {
            saw_call: false,
            instantiations: Vec::new(),
        }
    }

    fn from_context(context: InstantiationScanContext<'_>) -> Self {
        let (saw_call, instantiations) = context.finish();
        Self {
            saw_call,
            instantiations,
        }
    }
}

#[cfg(test)]
fn collect_public_function_instantiations(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
    function: &FunctionDecl,
) -> BTreeSet<TypeSubstitutions> {
    let function_bindings = FunctionTypeBindings::new();
    let enum_bindings =
        collect_root_call_enum_type_bindings(root_module, module_import_path, dependency_module);
    let struct_bindings =
        collect_root_call_struct_type_bindings(root_module, module_import_path, dependency_module);
    collect_public_function_call_instantiations(
        root_module,
        module_import_path,
        function,
        &function_bindings,
        &enum_bindings,
        &struct_bindings,
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
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    collect_public_function_call_instantiation_status(
        root_module,
        module_import_path,
        function,
        function_bindings,
        enum_bindings,
        struct_bindings,
    )
    .instantiations
}

pub(super) fn collect_public_function_call_instantiation_status(
    root_module: &Module,
    module_import_path: &[String],
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
) -> PublicFunctionCallInstantiations {
    let local_names =
        dependency_imported_local_names(root_module, module_import_path, function.name.as_str());
    if local_names.is_empty() {
        return PublicFunctionCallInstantiations::none();
    }

    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut context = InstantiationScanContext::new(
        &local_names,
        function,
        function_bindings,
        enum_bindings,
        struct_bindings,
    );
    scan_root_module_items(root_module, &root_bindings, &mut context, |_| true);
    PublicFunctionCallInstantiations::from_context(context)
}

pub(super) fn collect_local_function_call_instantiations(
    root_module: &Module,
    function: &FunctionDecl,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let local_names = BTreeSet::from([function.name.clone()]);
    let root_bindings = collect_root_value_type_bindings(root_module);
    let mut context = InstantiationScanContext::new(
        &local_names,
        function,
        function_bindings,
        enum_bindings,
        struct_bindings,
    );
    scan_root_module_items(root_module, &root_bindings, &mut context, |function| {
        function.generics.is_empty()
    });
    let (_, instantiations) = context.finish();
    instantiations
}

fn scan_root_module_items(
    root_module: &Module,
    root_bindings: &ValueTypeBindings,
    context: &mut InstantiationScanContext<'_>,
    should_scan_function: impl Fn(&FunctionDecl) -> bool,
) {
    for item in &root_module.items {
        if should_scan_root_item(item, &should_scan_function) {
            collect_dependency_generic_function_instantiations_from_item(
                item,
                root_bindings,
                context,
            );
        }
    }
}

fn should_scan_root_item(
    item: &Item,
    should_scan_function: &impl Fn(&FunctionDecl) -> bool,
) -> bool {
    match &item.kind {
        ItemKind::Function(function) => should_scan_function(function),
        _ => true,
    }
}

pub(super) fn collect_specialized_body_call_instantiations(
    caller_function: &FunctionDecl,
    target_function: &FunctionDecl,
    caller_substitutions: &TypeSubstitutions,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
) -> Vec<PublicFunctionCallInstantiation> {
    let local_names = BTreeSet::from([target_function.name.clone()]);
    collect_specialized_body_call_instantiations_for_local_names(
        caller_function,
        target_function,
        &local_names,
        caller_substitutions,
        function_bindings,
        enum_bindings,
        struct_bindings,
    )
}

pub(super) fn collect_specialized_body_call_instantiations_for_local_names(
    caller_function: &FunctionDecl,
    target_function: &FunctionDecl,
    local_names: &BTreeSet<String>,
    caller_substitutions: &TypeSubstitutions,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    struct_bindings: &StructTypeBindings,
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
    let mut context = InstantiationScanContext::new_with_type_substitutions(
        local_names,
        target_function,
        function_bindings,
        enum_bindings,
        struct_bindings,
        caller_substitutions,
    );
    collect_dependency_generic_function_instantiations_from_block(
        body,
        &mut bindings,
        &mut context,
        None,
        None,
    );
    let (_, instantiations) = context.finish();
    instantiations
}

#[cfg(test)]
#[path = "instantiations_tests.rs"]
mod tests;
