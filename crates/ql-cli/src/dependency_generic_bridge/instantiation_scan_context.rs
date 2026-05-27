use std::collections::BTreeSet;

use ql_ast::FunctionDecl;
use ql_span::Span;

use super::function_bindings::FunctionTypeBindings;
use super::substitutions::TypeSubstitutions;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicFunctionCallInstantiation {
    pub(super) callee_span: Span,
    pub(super) substitutions: TypeSubstitutions,
}

pub(super) struct InstantiationScanContext<'a> {
    pub(super) local_names: &'a BTreeSet<String>,
    pub(super) target_function: &'a FunctionDecl,
    pub(super) function_bindings: &'a FunctionTypeBindings,
    saw_call: bool,
    instantiations: Vec<PublicFunctionCallInstantiation>,
}

impl<'a> InstantiationScanContext<'a> {
    pub(super) fn new(
        local_names: &'a BTreeSet<String>,
        target_function: &'a FunctionDecl,
        function_bindings: &'a FunctionTypeBindings,
    ) -> Self {
        Self {
            local_names,
            target_function,
            function_bindings,
            saw_call: false,
            instantiations: Vec::new(),
        }
    }

    pub(super) fn mark_call_seen(&mut self) {
        self.saw_call = true;
    }

    pub(super) fn push_instantiation(
        &mut self,
        callee_span: Span,
        substitutions: TypeSubstitutions,
    ) {
        self.instantiations.push(PublicFunctionCallInstantiation {
            callee_span,
            substitutions,
        });
    }

    pub(super) fn finish(self) -> (bool, Vec<PublicFunctionCallInstantiation>) {
        (self.saw_call, self.instantiations)
    }
}
