use std::collections::BTreeSet;

use ql_ast::{self, FunctionDecl, ItemKind};
use ql_span::Span;

use super::function_bindings::FunctionTypeBindings;
use super::instantiation_block_scanner::collect_dependency_generic_function_instantiations_from_block;
use super::instantiation_expr_scanner::collect_dependency_generic_function_instantiations_from_expr;
use super::substitutions::TypeSubstitutions;
use super::value_bindings::{ValueTypeBindings, collect_function_param_type_bindings};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiation {
    pub(super) callee_span: Span,
    pub(super) substitutions: TypeSubstitutions,
}

pub(super) fn collect_dependency_generic_function_instantiations_from_item(
    item: &ql_ast::Item,
    local_names: &BTreeSet<String>,
    dependency_function: &FunctionDecl,
    root_bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    saw_call: &mut bool,
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
) {
    match &item.kind {
        ItemKind::Function(root_function) => {
            collect_callable_body_instantiations(
                root_function,
                local_names,
                dependency_function,
                root_bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
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
                saw_call,
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
                        saw_call,
                        instantiations,
                    );
                }
            }
        }
        ItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                collect_callable_body_instantiations(
                    method,
                    local_names,
                    dependency_function,
                    root_bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ItemKind::Impl(impl_block) => {
            for method in &impl_block.methods {
                collect_callable_body_instantiations(
                    method,
                    local_names,
                    dependency_function,
                    root_bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ItemKind::Extend(extend_block) => {
            for method in &extend_block.methods {
                collect_callable_body_instantiations(
                    method,
                    local_names,
                    dependency_function,
                    root_bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ItemKind::Enum(_) | ItemKind::TypeAlias(_) | ItemKind::ExternBlock(_) => {}
    }
}

fn collect_callable_body_instantiations(
    callable: &FunctionDecl,
    local_names: &BTreeSet<String>,
    dependency_function: &FunctionDecl,
    root_bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    saw_call: &mut bool,
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
) {
    let Some(body) = &callable.body else {
        return;
    };
    let mut bindings = root_bindings.clone();
    collect_function_param_type_bindings(callable, &mut bindings);
    collect_dependency_generic_function_instantiations_from_block(
        body,
        local_names,
        dependency_function,
        &mut bindings,
        function_bindings,
        saw_call,
        instantiations,
        callable.return_type.as_ref(),
        callable.return_type.as_ref(),
    );
}
