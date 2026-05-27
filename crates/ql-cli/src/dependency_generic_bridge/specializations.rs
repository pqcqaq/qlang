use std::collections::BTreeSet;

use ql_ast::{FunctionDecl, Module};

use super::function_bindings::{
    FunctionTypeBindings, collect_imported_function_type_bindings,
    collect_local_function_type_bindings,
};
use super::instantiations;
use super::rendering::dependency_public_function_specialized_local_forwarder_name;
use super::specialized_forwarders::{
    has_complete_generic_substitutions, render_public_function_specialized_forwarder,
};
use super::{
    PublicFunctionSpecializationRender, RenderedPublicFunctionSpecializations, SourceRewrite,
    SpecializationModule, supports_local_function_specialization,
    supports_public_function_specialization,
};

#[cfg(test)]
pub(crate) fn render_public_function_specializations(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
    dependency_module: &Module,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    match render_public_function_specialization_status(
        module_import_path,
        function,
        contents,
        root_module,
        dependency_module,
        rendered_specializations,
    ) {
        PublicFunctionSpecializationRender::Rendered(rendered) => Some(rendered),
        PublicFunctionSpecializationRender::NotCalled
        | PublicFunctionSpecializationRender::Unsupported => None,
    }
}

#[cfg(test)]
pub(crate) fn render_public_function_specialization_status(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
    dependency_module: &Module,
    rendered_specializations: &mut BTreeSet<String>,
) -> PublicFunctionSpecializationRender {
    render_public_function_specialization_status_with_context(
        module_import_path,
        function,
        contents,
        root_module,
        dependency_module,
        &[],
        rendered_specializations,
    )
}

pub(crate) fn render_public_function_specialization_status_with_context(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
    rendered_specializations: &mut BTreeSet<String>,
) -> PublicFunctionSpecializationRender {
    if !supports_public_function_specialization(function) || function.body.is_none() {
        return PublicFunctionSpecializationRender::Unsupported;
    }
    let dependency_function_bindings =
        collect_specialization_function_type_bindings(dependency_module, specialization_modules);
    let mut root_function_bindings = collect_local_function_type_bindings(root_module);
    root_function_bindings.extend(collect_imported_function_type_bindings(
        root_module,
        module_import_path,
        dependency_module,
    ));
    let call_instantiations = instantiations::collect_public_function_call_instantiation_status(
        root_module,
        module_import_path,
        function,
        &root_function_bindings,
    );
    if !call_instantiations.saw_call {
        return PublicFunctionSpecializationRender::NotCalled;
    }
    match render_function_specializations(
        module_import_path,
        function,
        contents,
        dependency_module,
        &dependency_function_bindings,
        specialization_modules,
        call_instantiations.instantiations,
        rendered_specializations,
    ) {
        Some(rendered) => PublicFunctionSpecializationRender::Rendered(rendered),
        None => PublicFunctionSpecializationRender::Unsupported,
    }
}

pub(crate) fn render_local_function_specializations(
    function: &FunctionDecl,
    contents: &str,
    root_module: &Module,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    if !supports_local_function_specialization(function) || function.body.is_none() {
        return None;
    }
    let module_import_path = root_module
        .package
        .as_ref()
        .map(|package| package.path.segments.as_slice())
        .unwrap_or(&[]);
    let call_instantiations = instantiations::collect_local_function_call_instantiations(
        root_module,
        function,
        &collect_local_function_type_bindings(root_module),
    );
    render_function_specializations(
        module_import_path,
        function,
        contents,
        root_module,
        &collect_local_function_type_bindings(root_module),
        &[],
        call_instantiations,
        rendered_specializations,
    )
}

fn render_function_specializations(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    specialization_module: &Module,
    function_bindings: &FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
    call_instantiations: Vec<instantiations::PublicFunctionCallInstantiation>,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    if call_instantiations.is_empty() {
        return None;
    }
    let mut concrete_instantiations = BTreeSet::new();
    for instantiation in &call_instantiations {
        if !has_complete_generic_substitutions(function, &instantiation.substitutions) {
            return None;
        }
        concrete_instantiations.insert(instantiation.substitutions.clone());
    }

    let mut declarations = Vec::new();
    for substitutions in &concrete_instantiations {
        render_public_function_specialized_forwarder(
            module_import_path,
            function,
            contents,
            specialization_module,
            function_bindings,
            specialization_modules,
            substitutions,
            rendered_specializations,
            &mut declarations,
        )?;
    }

    let call_rewrites = call_instantiations
        .into_iter()
        .map(|instantiation| SourceRewrite {
            span: instantiation.callee_span,
            replacement: dependency_public_function_specialized_local_forwarder_name(
                module_import_path,
                &function.name,
                function,
                &instantiation.substitutions,
            ),
        })
        .collect();

    Some(RenderedPublicFunctionSpecializations {
        declarations: declarations.join("\n\n"),
        call_rewrites,
    })
}

fn collect_specialization_function_type_bindings(
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) -> FunctionTypeBindings {
    let mut bindings = collect_local_function_type_bindings(dependency_module);
    for module in specialization_modules {
        bindings.extend(collect_local_function_type_bindings(module.module));
    }
    for target in specialization_modules {
        bindings.extend(collect_imported_function_type_bindings(
            dependency_module,
            target.module_import_path,
            target.module,
        ));
    }
    for caller in specialization_modules {
        for target in specialization_modules {
            bindings.extend(collect_imported_function_type_bindings(
                caller.module,
                target.module_import_path,
                target.module,
            ));
        }
    }
    bindings
}
