use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{FunctionDecl, ItemKind, Module};

use super::function_bindings::{FunctionTypeBindings, dependency_imported_local_names};
use super::instantiations;
use super::rendering::{
    apply_specialized_body_rewrites, dependency_public_function_specialized_local_forwarder_name,
    render_dependency_bridge_generic_params_with_substitutions,
    render_dependency_bridge_param_list_with_substitutions,
    render_dependency_bridge_return_suffix_with_substitutions, replace_generic_identifiers,
    span_text,
};
use super::{
    SourceRewrite, SpecializationModule, supports_local_function_specialization,
    supports_public_function_specialization,
};

pub(super) fn render_public_function_specialized_forwarder(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    specialization_module: &Module,
    function_bindings: &FunctionTypeBindings,
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
    collect_same_module_specialized_body_call_rewrites(
        module_import_path,
        function,
        contents,
        specialization_module,
        function_bindings,
        specialization_modules,
        substitutions,
        rendered_specializations,
        declarations,
        &mut body_call_rewrites,
    )?;
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

fn collect_same_module_specialized_body_call_rewrites(
    module_import_path: &[String],
    function: &FunctionDecl,
    contents: &str,
    specialization_module: &Module,
    function_bindings: &FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
    substitutions: &BTreeMap<String, String>,
    rendered_specializations: &mut BTreeSet<String>,
    declarations: &mut Vec<String>,
    body_call_rewrites: &mut Vec<SourceRewrite>,
) -> Option<()> {
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
            render_specialized_body_call_rewrite(
                SpecializedBodyCallTarget {
                    module_import_path,
                    contents,
                    module: specialization_module,
                    callee,
                },
                instantiation,
                function_bindings,
                specialization_modules,
                rendered_specializations,
                declarations,
                body_call_rewrites,
            )?;
        }
    }
    Some(())
}

fn collect_imported_specialized_body_call_rewrites(
    function: &FunctionDecl,
    specialization_module: &Module,
    function_bindings: &FunctionTypeBindings,
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
            let local_names = dependency_imported_local_names(
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
                render_specialized_body_call_rewrite(
                    SpecializedBodyCallTarget {
                        module_import_path: target_module.module_import_path,
                        contents: target_module.contents,
                        module: target_module.module,
                        callee,
                    },
                    instantiation,
                    function_bindings,
                    specialization_modules,
                    rendered_specializations,
                    declarations,
                    body_call_rewrites,
                )?;
            }
        }
    }
    Some(())
}

struct SpecializedBodyCallTarget<'a> {
    module_import_path: &'a [String],
    contents: &'a str,
    module: &'a Module,
    callee: &'a FunctionDecl,
}

fn render_specialized_body_call_rewrite(
    target: SpecializedBodyCallTarget<'_>,
    instantiation: instantiations::PublicFunctionCallInstantiation,
    function_bindings: &FunctionTypeBindings,
    specialization_modules: &[SpecializationModule<'_>],
    rendered_specializations: &mut BTreeSet<String>,
    declarations: &mut Vec<String>,
    body_call_rewrites: &mut Vec<SourceRewrite>,
) -> Option<()> {
    if !has_complete_generic_substitutions(target.callee, &instantiation.substitutions) {
        return None;
    }
    render_public_function_specialized_forwarder(
        target.module_import_path,
        target.callee,
        target.contents,
        target.module,
        function_bindings,
        specialization_modules,
        &instantiation.substitutions,
        rendered_specializations,
        declarations,
    )?;
    body_call_rewrites.push(SourceRewrite {
        span: instantiation.callee_span,
        replacement: dependency_public_function_specialized_local_forwarder_name(
            target.module_import_path,
            &target.callee.name,
            target.callee,
            &instantiation.substitutions,
        ),
    });
    Some(())
}

pub(super) fn has_complete_generic_substitutions(
    function: &FunctionDecl,
    substitutions: &BTreeMap<String, String>,
) -> bool {
    function
        .generics
        .iter()
        .all(|generic| substitutions.contains_key(&generic.name))
}
