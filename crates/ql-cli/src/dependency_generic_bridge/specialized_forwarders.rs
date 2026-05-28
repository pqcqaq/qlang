use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{FunctionDecl, Module};

use super::enum_bindings::EnumTypeBindings;
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

pub(super) struct SpecializedForwarderRenderContext<'a, 'm> {
    function_bindings: &'a FunctionTypeBindings,
    enum_bindings: &'a EnumTypeBindings,
    rendered_specializations: &'a mut BTreeSet<String>,
    declarations: &'a mut Vec<String>,
    specialization_modules: &'a [SpecializationModule<'m>],
}

impl<'a, 'm> SpecializedForwarderRenderContext<'a, 'm> {
    pub(super) fn new(
        function_bindings: &'a FunctionTypeBindings,
        enum_bindings: &'a EnumTypeBindings,
        specialization_modules: &'a [SpecializationModule<'m>],
        rendered_specializations: &'a mut BTreeSet<String>,
        declarations: &'a mut Vec<String>,
    ) -> Self {
        Self {
            function_bindings,
            enum_bindings,
            rendered_specializations,
            declarations,
            specialization_modules,
        }
    }

    pub(super) fn render_public_function_specialized_forwarder(
        &mut self,
        module_import_path: &[String],
        function: &FunctionDecl,
        contents: &str,
        specialization_module: &Module,
        substitutions: &BTreeMap<String, String>,
    ) -> Option<()> {
        let params = render_dependency_bridge_param_list_with_substitutions(
            function,
            contents,
            substitutions,
        );
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
        if !self
            .rendered_specializations
            .insert(specialized_name.clone())
        {
            return Some(());
        }

        let body_call_rewrites = self.collect_specialized_body_call_rewrites(
            module_import_path,
            function,
            contents,
            specialization_module,
            substitutions,
        )?;

        let body = render_specialized_forwarder_body(
            function,
            contents,
            substitutions,
            &body_call_rewrites,
        )?;
        let generic_params =
            render_dependency_bridge_generic_params_with_substitutions(function, substitutions);

        self.declarations.push(format!(
            "fn {specialized_name}{generic_params}({params}){return_suffix} {body}"
        ));
        Some(())
    }

    fn collect_specialized_body_call_rewrites(
        &mut self,
        module_import_path: &[String],
        function: &FunctionDecl,
        contents: &str,
        specialization_module: &Module,
        substitutions: &BTreeMap<String, String>,
    ) -> Option<Vec<SourceRewrite>> {
        let mut body_call_rewrites = Vec::new();
        self.collect_same_module_specialized_body_call_rewrites(
            module_import_path,
            function,
            contents,
            specialization_module,
            substitutions,
            &mut body_call_rewrites,
        )?;
        self.collect_imported_specialized_body_call_rewrites(
            function,
            specialization_module,
            substitutions,
            &mut body_call_rewrites,
        )?;
        Some(body_call_rewrites)
    }

    fn collect_same_module_specialized_body_call_rewrites(
        &mut self,
        module_import_path: &[String],
        function: &FunctionDecl,
        contents: &str,
        specialization_module: &Module,
        substitutions: &BTreeMap<String, String>,
        body_call_rewrites: &mut Vec<SourceRewrite>,
    ) -> Option<()> {
        let function_bindings = self.function_bindings;
        let enum_bindings = self.enum_bindings;
        for target in same_module_specialized_call_targets(
            module_import_path,
            contents,
            specialization_module,
        ) {
            let instantiations = instantiations::collect_specialized_body_call_instantiations(
                function,
                target.callee,
                substitutions,
                function_bindings,
                enum_bindings,
            );
            self.render_specialized_body_call_rewrites_for_callee(
                target,
                instantiations,
                body_call_rewrites,
            )?;
        }
        Some(())
    }

    fn collect_imported_specialized_body_call_rewrites(
        &mut self,
        function: &FunctionDecl,
        specialization_module: &Module,
        substitutions: &BTreeMap<String, String>,
        body_call_rewrites: &mut Vec<SourceRewrite>,
    ) -> Option<()> {
        let function_bindings = self.function_bindings;
        let enum_bindings = self.enum_bindings;
        for (target, local_names) in
            imported_specialized_call_targets(specialization_module, self.specialization_modules)
        {
            let instantiations =
                instantiations::collect_specialized_body_call_instantiations_for_local_names(
                    function,
                    target.callee,
                    &local_names,
                    substitutions,
                    function_bindings,
                    enum_bindings,
                );
            self.render_specialized_body_call_rewrites_for_callee(
                target,
                instantiations,
                body_call_rewrites,
            )?;
        }
        Some(())
    }

    fn render_specialized_body_call_rewrites_for_callee<I>(
        &mut self,
        target: SpecializedBodyCallTarget<'_>,
        instantiations: I,
        body_call_rewrites: &mut Vec<SourceRewrite>,
    ) -> Option<()>
    where
        I: IntoIterator<Item = instantiations::PublicFunctionCallInstantiation>,
    {
        for instantiation in instantiations {
            self.render_specialized_body_call_rewrite(
                target.clone(),
                instantiation,
                body_call_rewrites,
            )?;
        }
        Some(())
    }

    fn render_specialized_body_call_rewrite(
        &mut self,
        target: SpecializedBodyCallTarget<'_>,
        instantiation: instantiations::PublicFunctionCallInstantiation,
        body_call_rewrites: &mut Vec<SourceRewrite>,
    ) -> Option<()> {
        if !has_complete_generic_substitutions(target.callee, &instantiation.substitutions) {
            return None;
        }
        self.render_public_function_specialized_forwarder(
            target.module_import_path,
            target.callee,
            target.contents,
            target.module,
            &instantiation.substitutions,
        )?;
        body_call_rewrites.push(specialized_call_rewrite(
            target.module_import_path,
            target.callee,
            &instantiation.substitutions,
            instantiation.callee_span,
        ));
        Some(())
    }
}

#[derive(Clone)]
struct SpecializedBodyCallTarget<'a> {
    module_import_path: &'a [String],
    contents: &'a str,
    module: &'a Module,
    callee: &'a FunctionDecl,
}

fn same_module_specialized_call_targets<'a>(
    module_import_path: &'a [String],
    contents: &'a str,
    module: &'a Module,
) -> Vec<SpecializedBodyCallTarget<'a>> {
    specializable_functions(module, supports_local_function_specialization)
        .map(|callee| SpecializedBodyCallTarget {
            module_import_path,
            contents,
            module,
            callee,
        })
        .collect()
}

fn imported_specialized_call_targets<'modules>(
    caller_module: &Module,
    specialization_modules: &[SpecializationModule<'modules>],
) -> Vec<(SpecializedBodyCallTarget<'modules>, BTreeSet<String>)> {
    let mut targets = Vec::new();
    for target_module in specialization_modules.iter().copied() {
        for callee in specializable_functions(
            target_module.module,
            supports_public_function_specialization,
        ) {
            let local_names = dependency_imported_local_names(
                caller_module,
                target_module.module_import_path,
                callee.name.as_str(),
            );
            if local_names.is_empty() {
                continue;
            }
            targets.push((
                SpecializedBodyCallTarget {
                    module_import_path: target_module.module_import_path,
                    contents: target_module.contents,
                    module: target_module.module,
                    callee,
                },
                local_names,
            ));
        }
    }
    targets
}

fn specializable_functions<'a>(
    module: &'a Module,
    supports: impl Fn(&FunctionDecl) -> bool + 'a,
) -> impl Iterator<Item = &'a FunctionDecl> + 'a {
    module
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ql_ast::ItemKind::Function(function) => Some(function),
            _ => None,
        })
        .filter(move |function| supports(function) && function.body.is_some())
}

fn render_specialized_forwarder_body(
    function: &FunctionDecl,
    contents: &str,
    substitutions: &BTreeMap<String, String>,
    body_call_rewrites: &[SourceRewrite],
) -> Option<String> {
    let body_span = function.body.as_ref()?.span;
    let body_source = span_text(contents, body_span);
    let leading_trim = body_source.len() - body_source.trim_start().len();
    let body_start = body_span.start + leading_trim;
    let body = apply_specialized_body_rewrites(body_source.trim(), body_start, body_call_rewrites);
    Some(replace_generic_identifiers(&body, substitutions))
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

pub(super) fn specialized_call_rewrite(
    module_import_path: &[String],
    function: &FunctionDecl,
    substitutions: &BTreeMap<String, String>,
    span: ql_span::Span,
) -> SourceRewrite {
    SourceRewrite {
        span,
        replacement: dependency_public_function_specialized_local_forwarder_name(
            module_import_path,
            &function.name,
            function,
            substitutions,
        ),
    }
}
