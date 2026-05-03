mod ids;
mod resolver;

use std::collections::{HashMap, HashSet};

use ql_ast::{Path, UseDecl, UseItem};
use ql_diagnostics::Diagnostic;
use ql_hir::{BlockId, ExprId, FunctionRef, ItemId, LocalId, PatternId, TypeId};
use ql_span::Span;

pub use ids::ScopeId;
pub use resolver::resolve_module;

/// Result of the Phase 2 name-resolution pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolutionMap {
    pub diagnostics: Vec<Diagnostic>,
    pub scopes: ScopeGraph,
    value_paths: HashMap<ExprId, ValueResolution>,
    type_paths: HashMap<TypeId, TypeResolution>,
    pattern_paths: HashMap<PatternId, ValueResolution>,
    struct_literal_paths: HashMap<ExprId, TypeResolution>,
    block_scopes: HashMap<BlockId, ScopeId>,
    expr_scopes: HashMap<ExprId, ScopeId>,
    pattern_scopes: HashMap<PatternId, ScopeId>,
    type_scopes: HashMap<TypeId, ScopeId>,
    item_scopes: HashMap<ItemId, ScopeId>,
    function_scopes: HashMap<Span, ScopeId>,
    async_function_scopes: HashSet<ScopeId>,
}

impl ResolutionMap {
    /// Return the resolved root binding for an expression-path use.
    ///
    /// This is intentionally conservative today: for `self.value` or `Command.Retry`,
    /// the stored resolution points at the root name (`self`, `Command`) instead of
    /// claiming that the full member path has already been semantically validated.
    pub fn expr_resolution(&self, expr_id: ExprId) -> Option<&ValueResolution> {
        self.value_paths.get(&expr_id)
    }

    /// Return the resolved root binding for a named type expression.
    pub fn type_resolution(&self, type_id: TypeId) -> Option<&TypeResolution> {
        self.type_paths.get(&type_id)
    }

    /// Return the resolved root binding for a pattern path such as `Command.Retry(...)`.
    pub fn pattern_resolution(&self, pattern_id: PatternId) -> Option<&ValueResolution> {
        self.pattern_paths.get(&pattern_id)
    }

    /// Return the resolved root type for a struct literal such as `User { ... }`.
    pub fn struct_literal_resolution(&self, expr_id: ExprId) -> Option<&TypeResolution> {
        self.struct_literal_paths.get(&expr_id)
    }

    pub fn block_scope(&self, block_id: BlockId) -> Option<ScopeId> {
        self.block_scopes.get(&block_id).copied()
    }

    pub fn expr_scope(&self, expr_id: ExprId) -> Option<ScopeId> {
        self.expr_scopes.get(&expr_id).copied()
    }

    pub fn pattern_scope(&self, pattern_id: PatternId) -> Option<ScopeId> {
        self.pattern_scopes.get(&pattern_id).copied()
    }

    pub fn type_scope(&self, type_id: TypeId) -> Option<ScopeId> {
        self.type_scopes.get(&type_id).copied()
    }

    /// Return the item-local scope allocated while resolving one top-level item.
    pub fn item_scope(&self, item_id: ItemId) -> Option<ScopeId> {
        self.item_scopes.get(&item_id).copied()
    }

    /// Return the function-local scope allocated while resolving one function or method body/signature.
    pub fn function_scope(&self, span: Span) -> Option<ScopeId> {
        self.function_scopes.get(&span).copied()
    }

    /// Return whether `scope_id` is enclosed by an `async fn` scope.
    pub fn scope_is_in_async_function(&self, scope_id: ScopeId) -> bool {
        let mut next = Some(scope_id);
        while let Some(current) = next {
            if self.async_function_scopes.contains(&current) {
                return true;
            }
            let scope = self.scopes.scope(current);
            if matches!(scope.kind, ScopeKind::Closure) {
                return false;
            }
            next = scope.parent;
        }
        false
    }

    /// Return whether the expression is enclosed by an `async fn`.
    pub fn expr_is_in_async_function(&self, expr_id: ExprId) -> bool {
        self.expr_scope(expr_id)
            .map(|scope| self.scope_is_in_async_function(scope))
            .unwrap_or(false)
    }
}

/// Lexical scopes produced while walking the HIR tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeGraph {
    scopes: Vec<Scope>,
}

impl ScopeGraph {
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    pub fn scope(&self, scope_id: ScopeId) -> &Scope {
        &self.scopes[scope_id.index()]
    }

    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    pub(crate) fn push(&mut self, scope: Scope) -> ScopeId {
        let id = ScopeId::from_index(self.scopes.len());
        self.scopes.push(scope);
        id
    }

    pub(crate) fn scope_mut(&mut self, scope_id: ScopeId) -> &mut Scope {
        &mut self.scopes[scope_id.index()]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub value_bindings: Vec<NamedValueBinding>,
    pub type_bindings: Vec<NamedTypeBinding>,
}

impl Scope {
    pub(crate) fn new(kind: ScopeKind, parent: Option<ScopeId>) -> Self {
        Self {
            kind,
            parent,
            value_bindings: Vec::new(),
            type_bindings: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Module,
    Item,
    Block,
    Closure,
    MatchArm,
    ForLoop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedValueBinding {
    pub name: String,
    pub resolution: ValueResolution,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedTypeBinding {
    pub name: String,
    pub resolution: TypeResolution,
}

/// One source-backed import binding exposed through the resolver.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportBinding {
    pub path: Path,
    pub local_name: String,
    pub definition_span: Span,
}

impl ImportBinding {
    pub fn direct(use_decl: &UseDecl) -> Self {
        let local_name = use_decl
            .alias
            .clone()
            .unwrap_or_else(|| use_decl.prefix.segments.last().cloned().unwrap_or_default());
        let definition_span = use_decl
            .alias_span
            .unwrap_or_else(|| use_decl.prefix.last_segment_span().unwrap_or_default());

        Self {
            path: use_decl.prefix.clone(),
            local_name,
            definition_span,
        }
    }

    pub fn grouped(prefix: &Path, item: &UseItem) -> Self {
        let mut path = prefix.clone();
        path.segments.push(item.name.clone());
        path.segment_spans.push(item.name_span);

        Self {
            path,
            local_name: item.alias.clone().unwrap_or_else(|| item.name.clone()),
            definition_span: item.alias_span.unwrap_or(item.name_span),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParamBinding {
    pub scope: ScopeId,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GenericBinding {
    pub scope: ScopeId,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinType {
    Bool,
    Char,
    String,
    Bytes,
    Void,
    Never,
    Int,
    UInt,
    I8,
    I16,
    I32,
    I64,
    ISize,
    U8,
    U16,
    U32,
    U64,
    USize,
    F32,
    F64,
}

/// Value-namespace bindings produced by the current best-effort resolver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueResolution {
    Local(LocalId),
    Param(ParamBinding),
    ArrayLengthGeneric(GenericBinding),
    SelfValue,
    Function(FunctionRef),
    Item(ItemId),
    Import(ImportBinding),
}

/// Type-namespace bindings produced by the current best-effort resolver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeResolution {
    Generic(GenericBinding),
    Builtin(BuiltinType),
    Item(ItemId),
    Import(ImportBinding),
}
