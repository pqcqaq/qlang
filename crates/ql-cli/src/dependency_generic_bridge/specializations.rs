use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{FunctionDecl, ItemKind, Module};

use super::instantiations;
use super::rendering::{
    apply_specialized_body_rewrites, dependency_public_function_specialized_local_forwarder_name,
    render_dependency_bridge_generic_params_with_substitutions,
    render_dependency_bridge_param_list_with_substitutions,
    render_dependency_bridge_return_suffix_with_substitutions, replace_generic_identifiers,
    span_text,
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
    let mut root_function_bindings =
        instantiations::collect_local_function_type_bindings(root_module);
    root_function_bindings.extend(instantiations::collect_imported_function_type_bindings(
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
        &instantiations::collect_local_function_type_bindings(root_module),
    );
    render_function_specializations(
        module_import_path,
        function,
        contents,
        root_module,
        &instantiations::collect_local_function_type_bindings(root_module),
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
    function_bindings: &instantiations::FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
    call_instantiations: Vec<instantiations::PublicFunctionCallInstantiation>,
    rendered_specializations: &mut BTreeSet<String>,
) -> Option<RenderedPublicFunctionSpecializations> {
    if call_instantiations.is_empty() {
        return None;
    }
    let mut concrete_instantiations = BTreeSet::new();
    for instantiation in &call_instantiations {
        if function
            .generics
            .iter()
            .any(|generic| !instantiation.substitutions.contains_key(&generic.name))
        {
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

fn render_public_function_specialized_forwarder(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    specialization_module: &Module,
    function_bindings: &instantiations::FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
    substitutions: &BTreeMap<String, String>,
    rendered_specializations: &mut BTreeSet<String>,
    declarations: &mut Vec<String>,
) -> Option<()> {
    let params =
        render_dependency_bridge_param_list_with_substitutions(function, contents, substitutions);
    let return_suffix = render_dependency_bridge_return_suffix_with_substitutions(
        function,
        contents,
        substitutions,
    );
    let specialized_name = dependency_public_function_specialized_local_forwarder_name(
        module_import_path,
        &function.name,
        function,
        substitutions,
    );
    if !rendered_specializations.insert(specialized_name.clone()) {
        return Some(());
    }

    let mut body_call_rewrites = Vec::new();
    for item in &specialization_module.items {
        let ItemKind::Function(callee) = &item.kind else {
            continue;
        };
        if !supports_local_function_specialization(callee) || callee.body.is_none() {
            continue;
        }
        for instantiation in instantiations::collect_specialized_body_call_instantiations(
            function,
            callee,
            substitutions,
            function_bindings,
        ) {
            if callee
                .generics
                .iter()
                .any(|generic| !instantiation.substitutions.contains_key(&generic.name))
            {
                return None;
            }
            render_public_function_specialized_forwarder(
                module_import_path,
                callee,
                contents,
                specialization_module,
                function_bindings,
                specialization_modules,
                &instantiation.substitutions,
                rendered_specializations,
                declarations,
            )?;
            body_call_rewrites.push(SourceRewrite {
                span: instantiation.callee_span,
                replacement: dependency_public_function_specialized_local_forwarder_name(
                    module_import_path,
                    &callee.name,
                    callee,
                    &instantiation.substitutions,
                ),
            });
        }
    }
    collect_imported_specialized_body_call_rewrites(
        function,
        specialization_module,
        function_bindings,
        specialization_modules,
        substitutions,
        rendered_specializations,
        declarations,
        &mut body_call_rewrites,
    )?;

    let body_span = function.body.as_ref()?.span;
    let body_source = span_text(contents, body_span);
    let leading_trim = body_source.len() - body_source.trim_start().len();
    let body_start = body_span.start + leading_trim;
    let body = apply_specialized_body_rewrites(body_source.trim(), body_start, &body_call_rewrites);
    let body = replace_generic_identifiers(&body, substitutions);
    let generic_params =
        render_dependency_bridge_generic_params_with_substitutions(function, substitutions);

    declarations.push(format!(
        "fn {specialized_name}{generic_params}({params}){return_suffix} {body}"
    ));
    Some(())
}

fn collect_specialization_function_type_bindings(
    dependency_module: &Module,
    specialization_modules: &[SpecializationModule<'_>],
) -> instantiations::FunctionTypeBindings {
    let mut bindings = instantiations::collect_local_function_type_bindings(dependency_module);
    for module in specialization_modules {
        bindings.extend(instantiations::collect_local_function_type_bindings(
            module.module,
        ));
    }
    for target in specialization_modules {
        bindings.extend(instantiations::collect_imported_function_type_bindings(
            dependency_module,
            target.module_import_path,
            target.module,
        ));
    }
    for caller in specialization_modules {
        for target in specialization_modules {
            bindings.extend(instantiations::collect_imported_function_type_bindings(
                caller.module,
                target.module_import_path,
                target.module,
            ));
        }
    }
    bindings
}

fn collect_imported_specialized_body_call_rewrites(
    function: &FunctionDecl,
    specialization_module: &Module,
    function_bindings: &instantiations::FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
    substitutions: &BTreeMap<String, String>,
    rendered_specializations: &mut BTreeSet<String>,
    declarations: &mut Vec<String>,
    body_call_rewrites: &mut Vec<SourceRewrite>,
) -> Option<()> {
    for target_module in specialization_modules {
        for item in &target_module.module.items {
            let ItemKind::Function(callee) = &item.kind else {
                continue;
            };
            if !supports_public_function_specialization(callee) || callee.body.is_none() {
                continue;
            }
            let local_names = instantiations::dependency_imported_local_names(
                specialization_module,
                target_module.module_import_path,
                callee.name.as_str(),
            );
            if local_names.is_empty() {
                continue;
            }
            for instantiation in
                instantiations::collect_specialized_body_call_instantiations_for_local_names(
                    function,
                    callee,
                    &local_names,
                    substitutions,
                    function_bindings,
                )
            {
                if callee
                    .generics
                    .iter()
                    .any(|generic| !instantiation.substitutions.contains_key(&generic.name))
                {
                    return None;
                }
                render_public_function_specialized_forwarder(
                    target_module.module_import_path,
                    callee,
                    target_module.contents,
                    target_module.module,
                    function_bindings,
                    specialization_modules,
                    &instantiation.substitutions,
                    rendered_specializations,
                    declarations,
                )?;
                body_call_rewrites.push(SourceRewrite {
                    span: instantiation.callee_span,
                    replacement: dependency_public_function_specialized_local_forwarder_name(
                        target_module.module_import_path,
                        &callee.name,
                        callee,
                        &instantiation.substitutions,
                    ),
                });
            }
        }
    }
    Some(())
}
