use std::collections::HashMap;

use ql_diagnostics::Diagnostic;
use ql_hir::{ExprId, ItemId, LocalId, Module, PatternId};
use ql_resolve::ResolutionMap;

use crate::{duplicates, typing};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldTarget {
    pub item_id: ItemId,
    pub field_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MethodTarget {
    pub item_id: ItemId,
    pub method_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemberTarget {
    Field(FieldTarget),
    Method(MethodTarget),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeckResult {
    diagnostics: Vec<Diagnostic>,
    expr_types: HashMap<ExprId, crate::Ty>,
    pattern_types: HashMap<PatternId, crate::Ty>,
    local_types: HashMap<LocalId, crate::Ty>,
    member_targets: HashMap<ExprId, MemberTarget>,
}

impl TypeckResult {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub fn expr_ty(&self, expr_id: ExprId) -> Option<&crate::Ty> {
        self.expr_types.get(&expr_id)
    }

    pub fn pattern_ty(&self, pattern_id: PatternId) -> Option<&crate::Ty> {
        self.pattern_types.get(&pattern_id)
    }

    pub fn local_ty(&self, local_id: LocalId) -> Option<&crate::Ty> {
        self.local_types.get(&local_id)
    }

    pub fn member_target(&self, expr_id: ExprId) -> Option<MemberTarget> {
        self.member_targets.get(&expr_id).copied()
    }
}

/// Run the current Phase 2 semantic checks over lowered HIR plus name-resolution data.
pub fn check_module(module: &Module, resolution: &ResolutionMap) -> Vec<Diagnostic> {
    analyze_module(module, resolution).into_diagnostics()
}

/// Run duplicate and first-pass typing checks and preserve queryable semantic results.
pub fn analyze_module(module: &Module, resolution: &ResolutionMap) -> TypeckResult {
    let typing = typing::analyze_module(module, resolution);
    let mut diagnostics = duplicates::check_module(module);
    diagnostics.extend(typing.diagnostics);

    TypeckResult {
        diagnostics,
        expr_types: typing.expr_types,
        pattern_types: typing.pattern_types,
        local_types: typing.local_types,
        member_targets: typing.member_targets,
    }
}
