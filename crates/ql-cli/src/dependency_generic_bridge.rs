#[cfg(test)]
use ql_ast::ItemKind;
use ql_ast::{FunctionDecl, Module, Param, Visibility};
#[cfg(test)]
use std::collections::BTreeSet;

mod call_args;
mod call_inference;
mod expr_inference;
mod function_bindings;
mod inferred_type_conversion;
mod inferred_type_predicates;
mod inferred_types;
mod instantiation_block_scanner;
mod instantiation_call_scanner;
mod instantiation_expr_scanner;
mod instantiation_scan_context;
mod instantiation_scanner;
mod instantiations;
mod rendering;
mod specialization_function_bindings;
mod specializations;
mod specialized_forwarders;
mod substitutions;
mod value_bindings;

pub(crate) use self::specializations::{
    render_local_function_specializations,
    render_public_function_specialization_status_with_context,
};

#[cfg(test)]
pub(crate) use self::specializations::render_public_function_specializations;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRewrite {
    pub span: ql_span::Span,
    pub replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedPublicFunctionSpecializations {
    pub declarations: String,
    pub call_rewrites: Vec<SourceRewrite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicFunctionSpecializationRender {
    NotCalled,
    Unsupported,
    Rendered(RenderedPublicFunctionSpecializations),
}

#[derive(Clone, Copy)]
pub struct SpecializationModule<'a> {
    pub module_import_path: &'a [String],
    pub contents: &'a str,
    pub module: &'a Module,
}

pub fn supports_public_function_specialization(function: &FunctionDecl) -> bool {
    function.visibility == Visibility::Public && supports_local_function_specialization(function)
}

pub fn supports_local_function_specialization(function: &FunctionDecl) -> bool {
    function.abi.is_none()
        && !function.is_async
        && !function.is_unsafe
        && !function.generics.is_empty()
        && function.where_clause.is_empty()
        && function
            .params
            .iter()
            .all(|param| matches!(param, Param::Regular { .. }))
}

#[cfg(test)]
#[path = "dependency_generic_bridge_tests.rs"]
mod tests;
