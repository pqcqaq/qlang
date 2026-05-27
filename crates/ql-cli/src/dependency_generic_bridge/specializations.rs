use std::collections::BTreeSet;

use ql_ast::{FunctionDecl, Module};

use super::function_bindings::{FunctionTypeBindings, collect_local_function_type_bindings};
use super::instantiations;
use super::specialization_function_bindings::{
    collect_root_call_function_type_bindings, collect_specialization_function_type_bindings,
};
use super::specialized_forwarders::{
    SpecializedForwarderRenderContext, has_complete_generic_substitutions, specialized_call_rewrite,
};
use super::{
    PublicFunctionSpecializationRender, RenderedPublicFunctionSpecializations,
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
    let root_function_bindings = collect_root_call_function_type_bindings(
        root_module,
        module_import_path,
        dependency_module,
    );
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
    let mut forwarder_context = SpecializedForwarderRenderContext::new(
        function_bindings,
        specialization_modules,
        rendered_specializations,
        &mut declarations,
    );
    for substitutions in &concrete_instantiations {
        forwarder_context.render_public_function_specialized_forwarder(
            module_import_path,
            function,
            contents,
            specialization_module,
            substitutions,
        )?;
    }

    let call_rewrites = call_instantiations
        .into_iter()
        .map(|instantiation| {
            specialized_call_rewrite(
                module_import_path,
                function,
                &instantiation.substitutions,
                instantiation.callee_span,
            )
        })
        .collect();

    Some(RenderedPublicFunctionSpecializations {
        declarations: declarations.join("\n\n"),
        call_rewrites,
    })
}
