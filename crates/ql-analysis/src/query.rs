use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use ql_ast::{Path, ReceiverKind, UnaryOp};
use ql_hir::{
    BlockId, EnumVariant, ExprId, ExprKind, Field, Function, FunctionRef, GenericParam, ItemId,
    ItemKind, LocalId, MatchArm, Module, Param, PatternId, PatternKind, TypeAlias, TypeId,
    TypeKind, VariantFields, WherePredicate,
};
use ql_lexer::{is_keyword, is_valid_identifier};
use ql_resolve::{
    BuiltinType, GenericBinding, ImportBinding, ParamBinding, ResolutionMap, ScopeId,
    TypeResolution, ValueResolution,
};
use ql_span::Span;
use ql_typeck::{FieldTarget, MemberTarget, MethodTarget, TypeckResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Const,
    Static,
    Struct,
    Enum,
    Variant,
    Trait,
    TypeAlias,
    Field,
    Method,
    Local,
    Parameter,
    Generic,
    SelfParameter,
    BuiltinType,
    Import,
}

impl SymbolKind {
    fn supports_same_file_rename(self) -> bool {
        matches!(
            self,
            Self::Function
                | Self::Const
                | Self::Static
                | Self::Struct
                | Self::Enum
                | Self::Variant
                | Self::Trait
                | Self::TypeAlias
                | Self::Field
                | Self::Method
                | Self::Local
                | Self::Parameter
                | Self::Generic
                | Self::Import
        )
    }
}

/// Source-backed definition target for go-to-definition style queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionTarget {
    pub kind: SymbolKind,
    pub name: String,
    pub span: Span,
}

/// One indexed reference site for a semantic symbol within the current source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceTarget {
    pub kind: SymbolKind,
    pub name: String,
    pub span: Span,
    pub is_definition: bool,
}

/// Rename-ready source span for the symbol under the current cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameTarget {
    pub kind: SymbolKind,
    pub name: String,
    pub span: Span,
}

/// One semantic completion candidate visible at the current cursor site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub insert_text: String,
    pub kind: SymbolKind,
    pub detail: String,
    pub ty: Option<String>,
}

/// One source-backed semantic-token occurrence for editor coloring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTokenOccurrence {
    pub span: Span,
    pub kind: SymbolKind,
}

/// Async operator forms represented in same-file semantic queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncOperatorKind {
    Await,
    Spawn,
    ForAwait,
}

/// Async semantic context for one source operator occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsyncContextInfo {
    pub span: Span,
    pub operator: AsyncOperatorKind,
    pub in_async_function: bool,
}

/// One same-file text edit for a rename operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameEdit {
    pub span: Span,
    pub replacement: String,
}

/// Same-file rename result returned by the semantic query layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameResult {
    pub kind: SymbolKind,
    pub old_name: String,
    pub new_name: String,
    pub edits: Vec<RenameEdit>,
}

/// User-facing rename validation failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameError {
    InvalidIdentifier(String),
    Keyword(String),
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(name) => write!(
                f,
                "rename target `{name}` is not a valid identifier; use letters, digits, and underscores, and do not start with a digit"
            ),
            Self::Keyword(name) => write!(
                f,
                "rename target `{name}` is a reserved keyword; escape it with backticks if you really want this name"
            ),
        }
    }
}

impl std::error::Error for RenameError {}

/// Minimal semantic hover payload shared by CLI-side tests and future LSP work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverInfo {
    pub span: Span,
    pub kind: SymbolKind,
    pub name: String,
    pub detail: String,
    pub ty: Option<String>,
    pub definition_span: Option<Span>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct QueryIndex {
    occurrences: Vec<IndexedSymbol>,
    field_shorthand_occurrences: HashMap<FieldTarget, Vec<FieldShorthandOccurrence>>,
    binding_shorthand_occurrences: HashMap<SymbolKey, Vec<BindingShorthandOccurrence>>,
    async_contexts: Vec<AsyncContextInfo>,
    completion_sites: Vec<CompletionSite>,
    completion_scopes: HashMap<ScopeId, CompletionScope>,
    semantic_completion_sites: Vec<SemanticCompletionSite>,
}

impl QueryIndex {
    pub(crate) fn build(
        source: &str,
        module: &Module,
        resolution: &ResolutionMap,
        typeck: &TypeckResult,
    ) -> Self {
        let mut builder = QueryIndexBuilder::new(source, module, resolution, typeck);
        builder.index_definitions();
        builder.index_uses();
        builder.index_async_contexts();
        builder.index_completion_support();
        builder.finish()
    }

    pub(crate) fn symbol_at(&self, offset: usize) -> Option<HoverInfo> {
        self.occurrences
            .iter()
            .find(|entry| entry.span.contains(offset))
            .map(|entry| entry.hover.clone())
    }

    pub(crate) fn async_context_at(&self, offset: usize) -> Option<AsyncContextInfo> {
        self.async_contexts
            .iter()
            .find(|entry| entry.span.contains(offset))
            .cloned()
    }

    pub(crate) fn definition_at(&self, offset: usize) -> Option<DefinitionTarget> {
        self.symbol_at(offset).and_then(|info| {
            info.definition_span.map(|span| DefinitionTarget {
                kind: info.kind,
                name: info.name,
                span,
            })
        })
    }

    pub(crate) fn references_at(&self, offset: usize) -> Option<Vec<ReferenceTarget>> {
        let key = self.occurrence_at(offset).map(|entry| entry.key.clone())?;
        let references = self
            .occurrences_for_key(&key)
            .into_iter()
            .map(|entry| ReferenceTarget {
                kind: entry.hover.kind,
                name: entry.hover.name.clone(),
                span: entry.span,
                is_definition: entry.hover.definition_span == Some(entry.span),
            })
            .collect::<Vec<_>>();
        Some(references)
    }

    pub(crate) fn prepare_rename_at(&self, offset: usize) -> Option<RenameTarget> {
        let entry = self.occurrence_at(offset)?;
        entry
            .hover
            .kind
            .supports_same_file_rename()
            .then(|| RenameTarget {
                kind: entry.hover.kind,
                name: entry.hover.name.clone(),
                span: entry.span,
            })
    }

    pub(crate) fn rename_at(
        &self,
        offset: usize,
        new_name: &str,
    ) -> Result<Option<RenameResult>, RenameError> {
        validate_rename_text(new_name)?;

        let Some(entry) = self.occurrence_at(offset) else {
            return Ok(None);
        };
        if !entry.hover.kind.supports_same_file_rename() {
            return Ok(None);
        }

        let replacement = new_name.to_owned();
        let mut edits = self
            .occurrences_for_key(&entry.key)
            .into_iter()
            .map(|entry| RenameEdit {
                span: entry.span,
                replacement: replacement.clone(),
            })
            .collect::<Vec<_>>();

        if let SymbolKey::Field(target) = &entry.key
            && let Some(shorthand_occurrences) = self.field_shorthand_occurrences.get(target)
        {
            edits.extend(shorthand_occurrences.iter().map(|occurrence| RenameEdit {
                span: occurrence.span,
                replacement: format!("{replacement}: {}", occurrence.binding_text),
            }));
        }
        if let Some(shorthand_occurrences) = self.binding_shorthand_occurrences.get(&entry.key) {
            for occurrence in shorthand_occurrences {
                let replacement_text = if occurrence.label_text == replacement {
                    replacement.clone()
                } else {
                    format!("{}: {replacement}", occurrence.label_text)
                };

                if let Some(edit) = edits.iter_mut().find(|edit| edit.span == occurrence.span) {
                    edit.replacement = replacement_text;
                } else {
                    edits.push(RenameEdit {
                        span: occurrence.span,
                        replacement: replacement_text,
                    });
                }
            }
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));
        edits.dedup_by(|left, right| {
            left.span == right.span && left.replacement == right.replacement
        });

        Ok(Some(RenameResult {
            kind: entry.hover.kind,
            old_name: entry.hover.name.clone(),
            new_name: replacement,
            edits,
        }))
    }

    pub(crate) fn completions_at(&self, offset: usize) -> Option<Vec<CompletionItem>> {
        if let Some(site) = self.semantic_completion_site_at(offset) {
            return Some(site.items.clone());
        }

        let site = self.completion_site_at(offset)?;
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        let mut next = Some(site.scope);

        while let Some(scope_id) = next {
            let scope = self.completion_scopes.get(&scope_id)?;
            let bindings = match site.namespace {
                CompletionNamespace::Value => &scope.value_bindings,
                CompletionNamespace::Type => &scope.type_bindings,
            };

            for symbol in bindings {
                if seen.insert(symbol.name.clone()) {
                    items.push(completion_item_from_symbol(symbol));
                }
            }

            next = scope.parent;
        }

        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub(crate) fn semantic_tokens(&self) -> Vec<SemanticTokenOccurrence> {
        let mut tokens = self
            .occurrences
            .iter()
            .map(|entry| SemanticTokenOccurrence {
                span: entry.span,
                kind: entry.hover.kind,
            })
            .collect::<Vec<_>>();
        tokens.sort_by_key(|token| (token.span.start, token.span.end, token.kind as usize));
        tokens.dedup_by(|left, right| left.span == right.span && left.kind == right.kind);
        tokens
    }

    fn occurrence_at(&self, offset: usize) -> Option<&IndexedSymbol> {
        self.occurrences
            .iter()
            .find(|entry| entry.span.contains(offset))
    }

    fn occurrences_for_key(&self, key: &SymbolKey) -> Vec<&IndexedSymbol> {
        let mut occurrences = self
            .occurrences
            .iter()
            .filter(|entry| &entry.key == key)
            .collect::<Vec<_>>();
        occurrences.sort_by_key(|entry| (entry.span.start, entry.span.end));
        occurrences.dedup_by_key(|entry| entry.span);
        occurrences
    }

    fn completion_site_at(&self, offset: usize) -> Option<&CompletionSite> {
        self.completion_sites
            .iter()
            .filter(|site| completion_span_contains(site.span, offset))
            .min_by_key(|site| {
                (
                    site.span.len(),
                    completion_namespace_rank(site.namespace),
                    site.span.start,
                    site.span.end,
                )
            })
    }

    fn semantic_completion_site_at(&self, offset: usize) -> Option<&SemanticCompletionSite> {
        self.semantic_completion_sites
            .iter()
            .filter(|site| completion_span_contains(site.span, offset))
            .min_by_key(|site| (site.span.len(), site.span.start, site.span.end))
    }
}

#[derive(Clone, Debug)]
struct IndexedSymbol {
    span: Span,
    key: SymbolKey,
    hover: HoverInfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FieldShorthandOccurrence {
    span: Span,
    binding_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BindingShorthandOccurrence {
    span: Span,
    label_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionNamespace {
    Value,
    Type,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberCompletionSource {
    Impl,
    Extend,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompletionSite {
    span: Span,
    scope: ScopeId,
    namespace: CompletionNamespace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SemanticCompletionSite {
    span: Span,
    items: Vec<CompletionItem>,
}

#[derive(Clone, Debug, Default)]
struct CompletionScope {
    parent: Option<ScopeId>,
    value_bindings: Vec<SymbolData>,
    type_bindings: Vec<SymbolData>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SymbolData {
    key: SymbolKey,
    kind: SymbolKind,
    name: String,
    detail: String,
    ty: Option<String>,
    definition_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SymbolKey {
    Item(ItemId),
    Function(FunctionRef),
    Variant(VariantTarget),
    Field(FieldTarget),
    Method(MethodTarget),
    DefinitionSpan(Span),
    Local(LocalId),
    Param(ParamBinding),
    Generic(GenericBinding),
    SelfValue(ScopeId),
    BuiltinType(BuiltinType),
    Import(ImportBinding),
}

struct QueryIndexBuilder<'a> {
    source: &'a str,
    module: &'a Module,
    resolution: &'a ResolutionMap,
    typeck: &'a TypeckResult,
    occurrences: Vec<IndexedSymbol>,
    item_defs: HashMap<ItemId, SymbolData>,
    function_defs: HashMap<FunctionRef, SymbolData>,
    variant_defs: HashMap<VariantTarget, SymbolData>,
    field_defs: HashMap<FieldTarget, SymbolData>,
    method_defs: HashMap<MethodTarget, SymbolData>,
    local_defs: HashMap<LocalId, SymbolData>,
    param_defs: HashMap<ParamBinding, SymbolData>,
    generic_defs: HashMap<GenericBinding, SymbolData>,
    self_defs: HashMap<ScopeId, SymbolData>,
    import_defs: HashMap<ImportBinding, SymbolData>,
    field_shorthand_occurrences: HashMap<FieldTarget, Vec<FieldShorthandOccurrence>>,
    binding_shorthand_occurrences: HashMap<SymbolKey, Vec<BindingShorthandOccurrence>>,
    async_contexts: Vec<AsyncContextInfo>,
    completion_sites: Vec<CompletionSite>,
    completion_scopes: HashMap<ScopeId, CompletionScope>,
    module_completion_scope: Option<ScopeId>,
    semantic_completion_sites: Vec<SemanticCompletionSite>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct VariantTarget {
    item_id: ItemId,
    variant_index: usize,
}

impl<'a> QueryIndexBuilder<'a> {
    fn new(
        source: &'a str,
        module: &'a Module,
        resolution: &'a ResolutionMap,
        typeck: &'a TypeckResult,
    ) -> Self {
        Self {
            source,
            module,
            resolution,
            typeck,
            occurrences: Vec::new(),
            item_defs: HashMap::new(),
            function_defs: HashMap::new(),
            variant_defs: HashMap::new(),
            field_defs: HashMap::new(),
            method_defs: HashMap::new(),
            local_defs: HashMap::new(),
            param_defs: HashMap::new(),
            generic_defs: HashMap::new(),
            self_defs: HashMap::new(),
            import_defs: HashMap::new(),
            field_shorthand_occurrences: HashMap::new(),
            binding_shorthand_occurrences: HashMap::new(),
            async_contexts: Vec::new(),
            completion_sites: Vec::new(),
            completion_scopes: HashMap::new(),
            module_completion_scope: None,
            semantic_completion_sites: Vec::new(),
        }
    }

    fn finish(mut self) -> QueryIndex {
        self.occurrences
            .sort_by_key(|entry| (entry.span.len(), entry.span.start, entry.span.end));
        self.completion_sites.sort_by_key(|site| {
            (
                site.span.len(),
                completion_namespace_rank(site.namespace),
                site.span.start,
                site.span.end,
            )
        });
        self.completion_sites.dedup_by(|left, right| left == right);
        self.semantic_completion_sites
            .sort_by_key(|site| (site.span.len(), site.span.start, site.span.end));
        self.semantic_completion_sites
            .dedup_by(|left, right| left == right);
        for occurrences in self.field_shorthand_occurrences.values_mut() {
            occurrences.sort_by_key(|occurrence| (occurrence.span.start, occurrence.span.end));
            occurrences.dedup_by(|left, right| left.span == right.span);
        }
        for occurrences in self.binding_shorthand_occurrences.values_mut() {
            occurrences.sort_by_key(|occurrence| (occurrence.span.start, occurrence.span.end));
            occurrences.dedup_by(|left, right| left.span == right.span);
        }
        self.async_contexts
            .sort_by_key(|entry| (entry.span.len(), entry.span.start, entry.span.end));
        self.async_contexts
            .dedup_by(|left, right| left.span == right.span && left.operator == right.operator);

        QueryIndex {
            occurrences: self.occurrences,
            field_shorthand_occurrences: self.field_shorthand_occurrences,
            binding_shorthand_occurrences: self.binding_shorthand_occurrences,
            async_contexts: self.async_contexts,
            completion_sites: self.completion_sites,
            completion_scopes: self.completion_scopes,
            semantic_completion_sites: self.semantic_completion_sites,
        }
    }

    fn index_definitions(&mut self) {
        self.index_import_definitions();
        for &item_id in &self.module.items {
            self.index_item_definitions(item_id);
        }
    }

    fn index_uses(&mut self) {
        for &item_id in &self.module.items {
            self.index_item_uses(item_id);
        }
    }

    fn index_async_contexts(&mut self) {
        for &item_id in &self.module.items {
            self.index_item_async_contexts(item_id);
        }
    }

    fn index_item_async_contexts(&mut self, item_id: ItemId) {
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => self.index_function_async_contexts(function),
            ItemKind::Trait(trait_decl) => {
                for method in &trait_decl.methods {
                    self.index_function_async_contexts(method);
                }
            }
            ItemKind::Impl(impl_block) => {
                for method in &impl_block.methods {
                    self.index_function_async_contexts(method);
                }
            }
            ItemKind::Extend(extend_block) => {
                for method in &extend_block.methods {
                    self.index_function_async_contexts(method);
                }
            }
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    self.index_function_async_contexts(function);
                }
            }
            ItemKind::Const(_)
            | ItemKind::Static(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::TypeAlias(_) => {}
        }
    }

    fn index_function_async_contexts(&mut self, function: &Function) {
        if let Some(body) = function.body {
            self.index_block_async_contexts(body);
        }
    }

    fn index_block_async_contexts(&mut self, block_id: BlockId) {
        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                ql_hir::StmtKind::Let { value, .. } => self.index_expr_async_contexts(*value),
                ql_hir::StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.index_expr_async_contexts(*expr);
                    }
                }
                ql_hir::StmtKind::Defer(expr) => self.index_expr_async_contexts(*expr),
                ql_hir::StmtKind::Break | ql_hir::StmtKind::Continue => {}
                ql_hir::StmtKind::While { condition, body } => {
                    self.index_expr_async_contexts(*condition);
                    self.index_block_async_contexts(*body);
                }
                ql_hir::StmtKind::Loop { body } => self.index_block_async_contexts(*body),
                ql_hir::StmtKind::For {
                    is_await,
                    iterable,
                    body,
                    ..
                } => {
                    if *is_await {
                        self.record_for_await_context(stmt.span, *iterable);
                    }
                    self.index_expr_async_contexts(*iterable);
                    self.index_block_async_contexts(*body);
                }
                ql_hir::StmtKind::Expr { expr, .. } => self.index_expr_async_contexts(*expr),
            }
        }

        if let Some(expr) = block.tail {
            self.index_expr_async_contexts(expr);
        }
    }

    fn index_expr_async_contexts(&mut self, expr_id: ExprId) {
        let expr = self.module.expr(expr_id);
        match &expr.kind {
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.index_expr_async_contexts(item);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                self.index_block_async_contexts(*block_id);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.index_expr_async_contexts(*condition);
                self.index_block_async_contexts(*then_branch);
                if let Some(expr) = else_branch {
                    self.index_expr_async_contexts(*expr);
                }
            }
            ExprKind::Match { value, arms } => {
                self.index_expr_async_contexts(*value);
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        self.index_expr_async_contexts(guard);
                    }
                    self.index_expr_async_contexts(arm.body);
                }
            }
            ExprKind::Closure { body, .. } => self.index_expr_async_contexts(*body),
            ExprKind::Call { callee, args } => {
                self.index_expr_async_contexts(*callee);
                for arg in args {
                    match arg {
                        ql_hir::CallArg::Positional(expr) => self.index_expr_async_contexts(*expr),
                        ql_hir::CallArg::Named { value, .. } => {
                            self.index_expr_async_contexts(*value)
                        }
                    }
                }
            }
            ExprKind::Member { object, .. } => self.index_expr_async_contexts(*object),
            ExprKind::Bracket { target, items } => {
                self.index_expr_async_contexts(*target);
                for &item in items {
                    self.index_expr_async_contexts(item);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.index_expr_async_contexts(field.value);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.index_expr_async_contexts(*left);
                self.index_expr_async_contexts(*right);
            }
            ExprKind::Unary { op, expr } => {
                let operator = match op {
                    UnaryOp::Await => Some(AsyncOperatorKind::Await),
                    UnaryOp::Spawn => Some(AsyncOperatorKind::Spawn),
                    UnaryOp::Neg => None,
                };
                if let Some(operator) = operator {
                    self.record_async_context(expr_id, operator);
                }
                self.index_expr_async_contexts(*expr);
            }
            ExprKind::Question(expr) => self.index_expr_async_contexts(*expr),
        }
    }

    fn record_async_context(&mut self, expr_id: ExprId, operator: AsyncOperatorKind) {
        let expr = self.module.expr(expr_id);
        let span = self.root_span(expr.span);
        self.async_contexts.push(AsyncContextInfo {
            span,
            operator,
            in_async_function: self.resolution.expr_is_in_async_function(expr_id),
        });
    }

    fn record_for_await_context(&mut self, stmt_span: Span, iterable_expr: ExprId) {
        self.async_contexts.push(AsyncContextInfo {
            span: self.for_await_operator_span(stmt_span),
            operator: AsyncOperatorKind::ForAwait,
            in_async_function: self.resolution.expr_is_in_async_function(iterable_expr),
        });
    }

    fn for_await_operator_span(&self, stmt_span: Span) -> Span {
        let fallback = self.root_span(stmt_span);
        let Some(stmt_text) = self.source.get(stmt_span.start..stmt_span.end) else {
            return fallback;
        };

        let mut offset = skip_whitespace_prefix(stmt_text, 0);
        let Some(rest) = stmt_text.get(offset..) else {
            return fallback;
        };
        if !rest.starts_with("for") {
            return fallback;
        }

        offset += "for".len();
        offset = skip_whitespace_prefix(stmt_text, offset);
        let Some(rest) = stmt_text.get(offset..) else {
            return fallback;
        };
        if !rest.starts_with("await") {
            return fallback;
        }

        let start = stmt_span.start + offset;
        Span::new(start, start + "await".len())
    }

    fn index_completion_support(&mut self) {
        for &item_id in &self.module.items {
            self.index_item_completion_sites(item_id);
        }

        if let Some(scope) = self.module_completion_scope {
            self.record_completion_site(
                Span::new(0, self.source.len()),
                scope,
                CompletionNamespace::Value,
            );
        }
    }

    fn index_import_definitions(&mut self) {
        for use_decl in &self.module.uses {
            if let Some(group) = &use_decl.group {
                for item in group {
                    self.define_import(ImportBinding::grouped(&use_decl.prefix, item));
                }
            } else {
                self.define_import(ImportBinding::direct(use_decl));
            }
        }
    }

    fn index_item_definitions(&mut self, item_id: ItemId) {
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => {
                let symbol = self.define_item(
                    item_id,
                    SymbolKind::Function,
                    function.name.clone(),
                    function.name_span,
                    render_function_signature(self.module, function),
                    None,
                );
                self.function_defs
                    .insert(FunctionRef::Item(item_id), symbol);

                if let Some(scope) = self.resolution.item_scope(item_id) {
                    self.index_function_bindings(function, scope, None);
                }
                self.index_function_local_definitions(function);
            }
            ItemKind::Const(global) => {
                self.define_item(
                    item_id,
                    SymbolKind::Const,
                    global.name.clone(),
                    global.name_span,
                    format!(
                        "const {}: {}",
                        global.name,
                        render_type(self.module, global.ty)
                    ),
                    Some(render_type(self.module, global.ty)),
                );
                self.index_expr_local_definitions(global.value);
            }
            ItemKind::Static(global) => {
                self.define_item(
                    item_id,
                    SymbolKind::Static,
                    global.name.clone(),
                    global.name_span,
                    format!(
                        "static {}: {}",
                        global.name,
                        render_type(self.module, global.ty)
                    ),
                    Some(render_type(self.module, global.ty)),
                );
                self.index_expr_local_definitions(global.value);
            }
            ItemKind::Struct(struct_decl) => {
                self.define_item(
                    item_id,
                    SymbolKind::Struct,
                    struct_decl.name.clone(),
                    struct_decl.name_span,
                    render_struct_detail(
                        struct_decl.is_data,
                        &struct_decl.name,
                        &struct_decl.generics,
                    ),
                    None,
                );

                if let Some(scope) = self.resolution.item_scope(item_id) {
                    self.index_generic_bindings(scope, &struct_decl.generics);
                }
                for (field_index, field) in struct_decl.fields.iter().enumerate() {
                    self.define_field(
                        FieldTarget {
                            item_id,
                            field_index,
                        },
                        field,
                    );
                    if let Some(default) = field.default {
                        self.index_expr_local_definitions(default);
                    }
                }
            }
            ItemKind::Enum(enum_decl) => {
                self.define_item(
                    item_id,
                    SymbolKind::Enum,
                    enum_decl.name.clone(),
                    enum_decl.name_span,
                    format!(
                        "enum {}{}",
                        enum_decl.name,
                        render_generics(&enum_decl.generics)
                    ),
                    None,
                );

                if let Some(scope) = self.resolution.item_scope(item_id) {
                    self.index_generic_bindings(scope, &enum_decl.generics);
                }
                for (variant_index, variant) in enum_decl.variants.iter().enumerate() {
                    self.define_variant(
                        VariantTarget {
                            item_id,
                            variant_index,
                        },
                        &enum_decl.name,
                        variant,
                    );
                }
            }
            ItemKind::Trait(trait_decl) => {
                self.define_item(
                    item_id,
                    SymbolKind::Trait,
                    trait_decl.name.clone(),
                    trait_decl.name_span,
                    format!(
                        "trait {}{}",
                        trait_decl.name,
                        render_generics(&trait_decl.generics)
                    ),
                    None,
                );

                if let Some(scope) = self.resolution.item_scope(item_id) {
                    self.index_generic_bindings(scope, &trait_decl.generics);
                }
                for (method_index, method) in trait_decl.methods.iter().enumerate() {
                    self.define_method_site(
                        MethodTarget {
                            item_id,
                            method_index,
                        },
                        method,
                    );
                    if let Some(scope) = self.resolution.function_scope(method.span) {
                        self.index_function_bindings(method, scope, Some("Self".to_owned()));
                    }
                    self.index_function_local_definitions(method);
                }
            }
            ItemKind::Impl(impl_block) => {
                if let Some(scope) = self.resolution.item_scope(item_id) {
                    self.index_generic_bindings(scope, &impl_block.generics);
                }

                let receiver_ty = render_type(self.module, impl_block.target);
                for (method_index, method) in impl_block.methods.iter().enumerate() {
                    self.define_method_site(
                        MethodTarget {
                            item_id,
                            method_index,
                        },
                        method,
                    );
                    if let Some(scope) = self.resolution.function_scope(method.span) {
                        self.index_function_bindings(method, scope, Some(receiver_ty.clone()));
                    }
                    self.index_function_local_definitions(method);
                }
            }
            ItemKind::Extend(extend_block) => {
                let receiver_ty = render_type(self.module, extend_block.target);
                for (method_index, method) in extend_block.methods.iter().enumerate() {
                    self.define_method_site(
                        MethodTarget {
                            item_id,
                            method_index,
                        },
                        method,
                    );
                    if let Some(scope) = self.resolution.function_scope(method.span) {
                        self.index_function_bindings(method, scope, Some(receiver_ty.clone()));
                    }
                    self.index_function_local_definitions(method);
                }
            }
            ItemKind::TypeAlias(alias) => {
                self.define_item(
                    item_id,
                    SymbolKind::TypeAlias,
                    alias.name.clone(),
                    alias.name_span,
                    render_type_alias_detail(self.module, alias),
                    None,
                );

                if let Some(scope) = self.resolution.item_scope(item_id) {
                    self.index_generic_bindings(scope, &alias.generics);
                }
            }
            ItemKind::ExternBlock(extern_block) => {
                for (index, function) in extern_block.functions.iter().enumerate() {
                    self.define_function_site(
                        Some(FunctionRef::ExternBlockMember {
                            block: item_id,
                            index,
                        }),
                        function,
                    );
                    if let Some(scope) = self.resolution.function_scope(function.span) {
                        self.index_function_bindings(function, scope, None);
                    }
                }
            }
        }
    }

    fn index_item_uses(&mut self, item_id: ItemId) {
        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => self.index_function_uses(function),
            ItemKind::Const(global) | ItemKind::Static(global) => {
                self.index_type_use(global.ty);
                self.index_expr_use(global.value);
            }
            ItemKind::Struct(struct_decl) => {
                for field in &struct_decl.fields {
                    self.index_type_use(field.ty);
                    if let Some(default) = field.default {
                        self.index_expr_use(default);
                    }
                }
            }
            ItemKind::Enum(enum_decl) => {
                for variant in &enum_decl.variants {
                    match &variant.fields {
                        ql_hir::VariantFields::Unit => {}
                        ql_hir::VariantFields::Tuple(items) => {
                            for &type_id in items {
                                self.index_type_use(type_id);
                            }
                        }
                        ql_hir::VariantFields::Struct(fields) => {
                            for field in fields {
                                self.index_type_use(field.ty);
                            }
                        }
                    }
                }
            }
            ItemKind::Trait(trait_decl) => {
                for method in &trait_decl.methods {
                    self.index_function_uses(method);
                }
            }
            ItemKind::Impl(impl_block) => {
                if let Some(trait_ty) = impl_block.trait_ty {
                    self.index_type_use(trait_ty);
                }
                self.index_type_use(impl_block.target);
                self.index_where_clause(&impl_block.where_clause);
                for method in &impl_block.methods {
                    self.index_function_uses(method);
                }
            }
            ItemKind::Extend(extend_block) => {
                self.index_type_use(extend_block.target);
                for method in &extend_block.methods {
                    self.index_function_uses(method);
                }
            }
            ItemKind::TypeAlias(alias) => self.index_type_use(alias.ty),
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    self.index_function_uses(function);
                }
            }
        }
    }

    fn index_function_uses(&mut self, function: &Function) {
        for param in &function.params {
            if let Param::Regular(param) = param {
                self.index_type_use(param.ty);
            }
        }

        if let Some(return_type) = function.return_type {
            self.index_type_use(return_type);
        }
        self.index_where_clause(&function.where_clause);
        if let Some(body) = function.body {
            self.index_block_use(body);
        }
    }

    fn index_where_clause(&mut self, predicates: &[WherePredicate]) {
        for predicate in predicates {
            self.index_type_use(predicate.target);
        }
    }

    fn index_item_completion_sites(&mut self, item_id: ItemId) {
        if let Some(scope) = self.resolution.item_scope(item_id) {
            self.record_completion_site(
                self.module.item(item_id).span,
                scope,
                CompletionNamespace::Value,
            );
        }

        match &self.module.item(item_id).kind {
            ItemKind::Function(function) => self.index_function_completion_sites(function),
            ItemKind::Const(global) | ItemKind::Static(global) => {
                self.index_type_completion_sites(global.ty);
                self.index_expr_completion_sites(global.value);
            }
            ItemKind::Struct(struct_decl) => {
                for field in &struct_decl.fields {
                    self.index_type_completion_sites(field.ty);
                    if let Some(default) = field.default {
                        self.index_expr_completion_sites(default);
                    }
                }
            }
            ItemKind::Enum(enum_decl) => {
                for variant in &enum_decl.variants {
                    match &variant.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(items) => {
                            for &type_id in items {
                                self.index_type_completion_sites(type_id);
                            }
                        }
                        VariantFields::Struct(fields) => {
                            for field in fields {
                                self.index_type_completion_sites(field.ty);
                            }
                        }
                    }
                }
            }
            ItemKind::Trait(trait_decl) => {
                for method in &trait_decl.methods {
                    self.index_function_completion_sites(method);
                }
            }
            ItemKind::Impl(impl_block) => {
                if let Some(trait_ty) = impl_block.trait_ty {
                    self.index_type_completion_sites(trait_ty);
                }
                self.index_type_completion_sites(impl_block.target);
                for predicate in &impl_block.where_clause {
                    self.index_type_completion_sites(predicate.target);
                }
                for method in &impl_block.methods {
                    self.index_function_completion_sites(method);
                }
            }
            ItemKind::Extend(extend_block) => {
                self.index_type_completion_sites(extend_block.target);
                for method in &extend_block.methods {
                    self.index_function_completion_sites(method);
                }
            }
            ItemKind::TypeAlias(alias) => self.index_type_completion_sites(alias.ty),
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    self.index_function_completion_sites(function);
                }
            }
        }
    }

    fn index_function_completion_sites(&mut self, function: &Function) {
        if let Some(scope) = self.resolution.function_scope(function.span) {
            self.record_completion_site(function.span, scope, CompletionNamespace::Value);
        }

        for param in &function.params {
            if let Param::Regular(param) = param {
                self.index_type_completion_sites(param.ty);
            }
        }
        if let Some(return_type) = function.return_type {
            self.index_type_completion_sites(return_type);
        }
        for predicate in &function.where_clause {
            self.index_type_completion_sites(predicate.target);
        }
        if let Some(body) = function.body {
            self.index_block_completion_sites(body);
        }
    }

    fn index_block_completion_sites(&mut self, block_id: BlockId) {
        if let Some(scope) = self.resolution.block_scope(block_id) {
            self.record_completion_site(
                self.module.block(block_id).span,
                scope,
                CompletionNamespace::Value,
            );
        }

        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            match &self.module.stmt(stmt_id).kind {
                ql_hir::StmtKind::Let { pattern, value, .. } => {
                    self.index_pattern_completion_sites(*pattern);
                    self.index_expr_completion_sites(*value);
                }
                ql_hir::StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.index_expr_completion_sites(*expr);
                    }
                }
                ql_hir::StmtKind::Defer(expr) => self.index_expr_completion_sites(*expr),
                ql_hir::StmtKind::Break | ql_hir::StmtKind::Continue => {}
                ql_hir::StmtKind::While { condition, body } => {
                    self.index_expr_completion_sites(*condition);
                    self.index_block_completion_sites(*body);
                }
                ql_hir::StmtKind::Loop { body } => self.index_block_completion_sites(*body),
                ql_hir::StmtKind::For {
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    self.index_pattern_completion_sites(*pattern);
                    self.index_expr_completion_sites(*iterable);
                    self.index_block_completion_sites(*body);
                }
                ql_hir::StmtKind::Expr { expr, .. } => self.index_expr_completion_sites(*expr),
            }
        }

        if let Some(expr) = block.tail {
            self.index_expr_completion_sites(expr);
        }
    }

    fn index_pattern_completion_sites(&mut self, pattern_id: PatternId) {
        let pattern = self.module.pattern(pattern_id);
        if let Some(scope) = self.resolution.pattern_scope(pattern_id) {
            self.record_completion_site(pattern.span, scope, CompletionNamespace::Value);
        }

        match &pattern.kind {
            PatternKind::Binding(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
            PatternKind::Path(path) => {
                if let Some(resolution) = self.resolution.pattern_resolution(pattern_id)
                    && let Some(item_id) = self.enum_item_for_value_resolution(resolution)
                {
                    self.record_variant_path_completion_site(path, item_id);
                }
            }
            PatternKind::Tuple(items) => {
                for &item in items {
                    self.index_pattern_completion_sites(item);
                }
            }
            PatternKind::TupleStruct { path, items } => {
                if let Some(resolution) = self.resolution.pattern_resolution(pattern_id)
                    && let Some(item_id) = self.enum_item_for_value_resolution(resolution)
                {
                    self.record_variant_path_completion_site(path, item_id);
                }
                for &item in items {
                    self.index_pattern_completion_sites(item);
                }
            }
            PatternKind::Struct { path, fields, .. } => {
                if let Some(resolution) = self.resolution.pattern_resolution(pattern_id)
                    && let Some(item_id) = self.enum_item_for_value_resolution(resolution)
                {
                    self.record_variant_path_completion_site(path, item_id);
                }
                for field in fields {
                    self.index_pattern_completion_sites(field.pattern);
                }
            }
        }
    }

    fn index_expr_completion_sites(&mut self, expr_id: ExprId) {
        let expr = self.module.expr(expr_id);
        if let Some(scope) = self.resolution.expr_scope(expr_id) {
            self.record_completion_site(expr.span, scope, CompletionNamespace::Value);
        }

        match &expr.kind {
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.index_expr_completion_sites(item);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                self.index_block_completion_sites(*block_id);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.index_expr_completion_sites(*condition);
                self.index_block_completion_sites(*then_branch);
                if let Some(expr) = else_branch {
                    self.index_expr_completion_sites(*expr);
                }
            }
            ExprKind::Match { value, arms } => {
                self.index_expr_completion_sites(*value);
                for arm in arms {
                    self.index_pattern_completion_sites(arm.pattern);
                    if let Some(guard) = arm.guard {
                        self.index_expr_completion_sites(guard);
                    }
                    self.index_expr_completion_sites(arm.body);
                }
            }
            ExprKind::Closure { body, .. } => self.index_expr_completion_sites(*body),
            ExprKind::Call { callee, args } => {
                self.index_expr_completion_sites(*callee);
                for arg in args {
                    match arg {
                        ql_hir::CallArg::Positional(expr) => {
                            self.index_expr_completion_sites(*expr);
                        }
                        ql_hir::CallArg::Named { value, .. } => {
                            self.index_expr_completion_sites(*value);
                        }
                    }
                }
            }
            ExprKind::Member {
                object, field_span, ..
            } => {
                self.index_expr_completion_sites(*object);
                self.record_member_completion_site(*field_span, *object);
            }
            ExprKind::Bracket { target, items } => {
                self.index_expr_completion_sites(*target);
                for &item in items {
                    self.index_expr_completion_sites(item);
                }
            }
            ExprKind::StructLiteral { path, fields } => {
                if let Some(resolution) = self.resolution.struct_literal_resolution(expr_id)
                    && let Some(item_id) = self.enum_item_for_type_resolution(resolution)
                {
                    self.record_variant_path_completion_site(path, item_id);
                }
                for field in fields {
                    self.index_expr_completion_sites(field.value);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.index_expr_completion_sites(*left);
                self.index_expr_completion_sites(*right);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
                self.index_expr_completion_sites(*expr);
            }
        }
    }

    fn index_type_completion_sites(&mut self, type_id: TypeId) {
        let ty = self.module.ty(type_id);
        if let Some(scope) = self.resolution.type_scope(type_id) {
            self.record_completion_site(ty.span, scope, CompletionNamespace::Type);
        }

        match &ty.kind {
            TypeKind::Pointer { inner, .. } => self.index_type_completion_sites(*inner),
            TypeKind::Array { element, .. } => self.index_type_completion_sites(*element),
            TypeKind::Named { args, .. } => {
                for &arg in args {
                    self.index_type_completion_sites(arg);
                }
            }
            TypeKind::Tuple(items) => {
                for &item in items {
                    self.index_type_completion_sites(item);
                }
            }
            TypeKind::Callable { params, ret } => {
                for &param in params {
                    self.index_type_completion_sites(param);
                }
                self.index_type_completion_sites(*ret);
            }
        }
    }

    fn index_function_bindings(
        &mut self,
        function: &Function,
        scope: ScopeId,
        receiver_ty: Option<String>,
    ) {
        self.index_generic_bindings(scope, &function.generics);

        for (index, param) in function.params.iter().enumerate() {
            match param {
                Param::Regular(param) => {
                    let ty = render_type(self.module, param.ty);
                    self.define_param(
                        ParamBinding { scope, index },
                        param.name.clone(),
                        param.name_span,
                        format!("param {}: {}", param.name, ty),
                        Some(ty),
                    );
                }
                Param::Receiver(receiver) => {
                    self.define_self(scope, receiver.kind, receiver.span, receiver_ty.clone());
                }
            }
        }
    }

    fn index_generic_bindings(&mut self, scope: ScopeId, generics: &[GenericParam]) {
        for (index, generic) in generics.iter().enumerate() {
            if self
                .generic_defs
                .contains_key(&GenericBinding { scope, index })
            {
                continue;
            }

            let symbol = SymbolData {
                key: SymbolKey::Generic(GenericBinding { scope, index }),
                kind: SymbolKind::Generic,
                name: generic.name.clone(),
                detail: render_generic_detail(generic),
                ty: None,
                definition_span: Some(generic.name_span),
            };
            self.push_occurrence(generic.name_span, &symbol);
            self.generic_defs
                .insert(GenericBinding { scope, index }, symbol);
        }
    }

    fn index_function_local_definitions(&mut self, function: &Function) {
        if let Some(body) = function.body {
            self.index_block_local_definitions(body);
        }
    }

    fn index_block_local_definitions(&mut self, block_id: BlockId) {
        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                ql_hir::StmtKind::Let { pattern, value, .. } => {
                    self.index_pattern_local_definitions(*pattern);
                    self.index_expr_local_definitions(*value);
                }
                ql_hir::StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.index_expr_local_definitions(*expr);
                    }
                }
                ql_hir::StmtKind::Defer(expr) => self.index_expr_local_definitions(*expr),
                ql_hir::StmtKind::Break | ql_hir::StmtKind::Continue => {}
                ql_hir::StmtKind::While { condition, body } => {
                    self.index_expr_local_definitions(*condition);
                    self.index_block_local_definitions(*body);
                }
                ql_hir::StmtKind::Loop { body } => self.index_block_local_definitions(*body),
                ql_hir::StmtKind::For {
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    self.index_pattern_local_definitions(*pattern);
                    self.index_expr_local_definitions(*iterable);
                    self.index_block_local_definitions(*body);
                }
                ql_hir::StmtKind::Expr { expr, .. } => self.index_expr_local_definitions(*expr),
            }
        }

        if let Some(expr) = block.tail {
            self.index_expr_local_definitions(expr);
        }
    }

    fn index_pattern_local_definitions(&mut self, pattern_id: PatternId) {
        let pattern = self.module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(local_id) => self.define_local(*local_id),
            PatternKind::Tuple(items) | PatternKind::TupleStruct { items, .. } => {
                for &item in items {
                    self.index_pattern_local_definitions(item);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for field in fields {
                    self.index_pattern_local_definitions(field.pattern);
                }
            }
            PatternKind::Path(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
    }

    fn index_expr_local_definitions(&mut self, expr_id: ExprId) {
        let expr = self.module.expr(expr_id);
        match &expr.kind {
            ExprKind::Name(_)
            | ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.index_expr_local_definitions(item);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                self.index_block_local_definitions(*block_id);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.index_expr_local_definitions(*condition);
                self.index_block_local_definitions(*then_branch);
                if let Some(expr) = else_branch {
                    self.index_expr_local_definitions(*expr);
                }
            }
            ExprKind::Match { value, arms } => {
                self.index_expr_local_definitions(*value);
                for arm in arms {
                    self.index_match_arm_local_definitions(arm);
                }
            }
            ExprKind::Closure { params, body, .. } => {
                for &local_id in params {
                    self.define_local(local_id);
                }
                self.index_expr_local_definitions(*body);
            }
            ExprKind::Call { callee, args } => {
                self.index_expr_local_definitions(*callee);
                for arg in args {
                    match arg {
                        ql_hir::CallArg::Positional(expr) => {
                            self.index_expr_local_definitions(*expr);
                        }
                        ql_hir::CallArg::Named { value, .. } => {
                            self.index_expr_local_definitions(*value);
                        }
                    }
                }
            }
            ExprKind::Member { object, .. } => self.index_expr_local_definitions(*object),
            ExprKind::Bracket { target, items } => {
                self.index_expr_local_definitions(*target);
                for &item in items {
                    self.index_expr_local_definitions(item);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.index_expr_local_definitions(field.value);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.index_expr_local_definitions(*left);
                self.index_expr_local_definitions(*right);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
                self.index_expr_local_definitions(*expr);
            }
        }
    }

    fn index_match_arm_local_definitions(&mut self, arm: &MatchArm) {
        self.index_pattern_local_definitions(arm.pattern);
        if let Some(guard) = arm.guard {
            self.index_expr_local_definitions(guard);
        }
        self.index_expr_local_definitions(arm.body);
    }

    fn index_block_use(&mut self, block_id: BlockId) {
        let block = self.module.block(block_id);
        for &stmt_id in &block.statements {
            let stmt = self.module.stmt(stmt_id);
            match &stmt.kind {
                ql_hir::StmtKind::Let { pattern, value, .. } => {
                    self.index_pattern_use(*pattern);
                    self.index_expr_use(*value);
                }
                ql_hir::StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.index_expr_use(*expr);
                    }
                }
                ql_hir::StmtKind::Defer(expr) => self.index_expr_use(*expr),
                ql_hir::StmtKind::Break | ql_hir::StmtKind::Continue => {}
                ql_hir::StmtKind::While { condition, body } => {
                    self.index_expr_use(*condition);
                    self.index_block_use(*body);
                }
                ql_hir::StmtKind::Loop { body } => self.index_block_use(*body),
                ql_hir::StmtKind::For {
                    pattern,
                    iterable,
                    body,
                    ..
                } => {
                    self.index_pattern_use(*pattern);
                    self.index_expr_use(*iterable);
                    self.index_block_use(*body);
                }
                ql_hir::StmtKind::Expr { expr, .. } => self.index_expr_use(*expr),
            }
        }

        if let Some(expr) = block.tail {
            self.index_expr_use(expr);
        }
    }

    fn index_pattern_use(&mut self, pattern_id: PatternId) {
        let pattern = self.module.pattern(pattern_id);
        match &pattern.kind {
            PatternKind::Binding(_) => {}
            PatternKind::Tuple(items) => {
                for &item in items {
                    self.index_pattern_use(item);
                }
            }
            PatternKind::Path(path)
            | PatternKind::TupleStruct { path, .. }
            | PatternKind::Struct { path, .. } => {
                if let Some(resolution) = self.resolution.pattern_resolution(pattern_id)
                    && let Some(symbol) = self.symbol_for_value_resolution(
                        resolution,
                        self.resolution.pattern_scope(pattern_id),
                    )
                {
                    self.push_occurrence(self.path_root_span(path, pattern.span), &symbol);
                    self.index_variant_value_path_use(path, resolution);
                }

                match &pattern.kind {
                    PatternKind::TupleStruct { items, .. } => {
                        for &item in items {
                            self.index_pattern_use(item);
                        }
                    }
                    PatternKind::Struct { fields, .. } => {
                        let field_owner =
                            self.resolution
                                .pattern_resolution(pattern_id)
                                .and_then(|resolution| {
                                    self.struct_item_for_root_value_path(path, resolution)
                                });
                        for field in fields {
                            if let Some(item_id) = field_owner
                                && let Some(target) =
                                    self.field_target_for_struct_item(item_id, &field.name)
                            {
                                if field.is_shorthand {
                                    self.record_field_shorthand_occurrence(target, field.name_span);
                                    if let PatternKind::Binding(local_id) =
                                        &self.module.pattern(field.pattern).kind
                                    {
                                        self.record_binding_shorthand_occurrence(
                                            SymbolKey::Local(*local_id),
                                            field.name_span,
                                        );
                                    }
                                } else if let Some(symbol) = self.field_defs.get(&target).cloned() {
                                    self.push_occurrence(field.name_span, &symbol);
                                }
                            }
                            self.index_pattern_use(field.pattern);
                        }
                    }
                    _ => {}
                }
            }
            PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
    }

    fn index_expr_use(&mut self, expr_id: ExprId) {
        let expr = self.module.expr(expr_id);
        match &expr.kind {
            ExprKind::Name(_) => {
                if let Some(resolution) = self.resolution.expr_resolution(expr_id)
                    && let Some(symbol) = self.symbol_for_value_resolution(
                        resolution,
                        self.resolution.expr_scope(expr_id),
                    )
                {
                    self.push_occurrence(expr.span, &symbol);
                }
            }
            ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral => {}
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for &item in items {
                    self.index_expr_use(item);
                }
            }
            ExprKind::Block(block_id) | ExprKind::Unsafe(block_id) => {
                self.index_block_use(*block_id)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.index_expr_use(*condition);
                self.index_block_use(*then_branch);
                if let Some(expr) = else_branch {
                    self.index_expr_use(*expr);
                }
            }
            ExprKind::Match { value, arms } => {
                self.index_expr_use(*value);
                for arm in arms {
                    self.index_pattern_use(arm.pattern);
                    if let Some(guard) = arm.guard {
                        self.index_expr_use(guard);
                    }
                    self.index_expr_use(arm.body);
                }
            }
            ExprKind::Closure { body, .. } => self.index_expr_use(*body),
            ExprKind::Call { callee, args } => {
                self.index_expr_use(*callee);
                for arg in args {
                    match arg {
                        ql_hir::CallArg::Positional(expr) => self.index_expr_use(*expr),
                        ql_hir::CallArg::Named { value, .. } => self.index_expr_use(*value),
                    }
                }
            }
            ExprKind::Member {
                object,
                field,
                field_span,
            } => {
                self.index_expr_use(*object);
                if let Some(target) = self.typeck.member_target(expr_id)
                    && let Some(symbol) = self.symbol_for_member_target(target)
                {
                    self.push_occurrence(*field_span, &symbol);
                } else if matches!(self.module.expr(*object).kind, ExprKind::Name(_))
                    && let Some(resolution) = self.resolution.expr_resolution(expr_id)
                    && let Some(item_id) = self.enum_item_for_value_resolution(resolution)
                {
                    self.index_variant_member_use(item_id, field, *field_span);
                }
            }
            ExprKind::Bracket { target, items } => {
                self.index_expr_use(*target);
                for &item in items {
                    self.index_expr_use(item);
                }
            }
            ExprKind::StructLiteral { path, fields } => {
                let field_owner = self
                    .resolution
                    .struct_literal_resolution(expr_id)
                    .and_then(|resolution| self.struct_item_for_root_type_path(path, resolution));
                if let Some(resolution) = self.resolution.struct_literal_resolution(expr_id)
                    && let Some(symbol) = self.symbol_for_type_resolution(resolution)
                {
                    self.push_occurrence(self.path_root_span(path, expr.span), &symbol);
                    self.index_variant_type_path_use(path, resolution);
                }
                for field in fields {
                    if let Some(item_id) = field_owner
                        && let Some(target) =
                            self.field_target_for_struct_item(item_id, &field.name)
                    {
                        if field.is_shorthand {
                            self.record_field_shorthand_occurrence(target, field.name_span);
                            if let Some(resolution) = self.resolution.expr_resolution(field.value)
                                && let Some(symbol) = self.symbol_for_value_resolution(
                                    resolution,
                                    self.resolution.expr_scope(field.value),
                                )
                            {
                                self.record_binding_shorthand_occurrence(
                                    symbol.key,
                                    field.name_span,
                                );
                            }
                        } else if let Some(symbol) = self.field_defs.get(&target).cloned() {
                            self.push_occurrence(field.name_span, &symbol);
                        }
                    }
                    self.index_expr_use(field.value);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.index_expr_use(*left);
                self.index_expr_use(*right);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => self.index_expr_use(*expr),
        }
    }

    fn index_type_use(&mut self, type_id: TypeId) {
        let ty = self.module.ty(type_id);
        match &ty.kind {
            TypeKind::Pointer { inner, .. } => self.index_type_use(*inner),
            TypeKind::Array { element, .. } => self.index_type_use(*element),
            TypeKind::Named { path, args } => {
                if let Some(resolution) = self.resolution.type_resolution(type_id)
                    && let Some(symbol) = self.symbol_for_type_resolution(resolution)
                {
                    self.push_occurrence(self.path_root_span(path, ty.span), &symbol);
                }
                for &arg in args {
                    self.index_type_use(arg);
                }
            }
            TypeKind::Tuple(items) => {
                for &item in items {
                    self.index_type_use(item);
                }
            }
            TypeKind::Callable { params, ret } => {
                for &param in params {
                    self.index_type_use(param);
                }
                self.index_type_use(*ret);
            }
        }
    }

    fn define_item(
        &mut self,
        item_id: ItemId,
        kind: SymbolKind,
        name: String,
        span: Span,
        detail: String,
        ty: Option<String>,
    ) -> SymbolData {
        let symbol = SymbolData {
            key: SymbolKey::Item(item_id),
            kind,
            name,
            detail,
            ty,
            definition_span: Some(span),
        };
        self.push_occurrence(span, &symbol);
        self.item_defs.insert(item_id, symbol.clone());
        symbol
    }

    fn define_function_site(&mut self, function_ref: Option<FunctionRef>, function: &Function) {
        let symbol = SymbolData {
            key: function_ref
                .map(SymbolKey::Function)
                .unwrap_or(SymbolKey::DefinitionSpan(function.name_span)),
            kind: SymbolKind::Function,
            name: function.name.clone(),
            detail: render_function_signature(self.module, function),
            ty: None,
            definition_span: Some(function.name_span),
        };
        self.push_occurrence(function.name_span, &symbol);
        if let Some(function_ref) = function_ref {
            self.function_defs.insert(function_ref, symbol);
        }
    }

    fn define_variant(&mut self, target: VariantTarget, enum_name: &str, variant: &EnumVariant) {
        let symbol = SymbolData {
            key: SymbolKey::Variant(target),
            kind: SymbolKind::Variant,
            name: variant.name.clone(),
            detail: render_variant_detail(self.module, enum_name, variant),
            ty: Some(enum_name.to_owned()),
            definition_span: Some(variant.name_span),
        };
        self.push_occurrence(variant.name_span, &symbol);
        self.variant_defs.insert(target, symbol);
    }

    fn define_field(&mut self, target: FieldTarget, field: &Field) {
        let ty = render_type(self.module, field.ty);
        let symbol = SymbolData {
            key: SymbolKey::Field(target),
            kind: SymbolKind::Field,
            name: field.name.clone(),
            detail: format!("field {}: {}", field.name, ty),
            ty: Some(ty),
            definition_span: Some(field.name_span),
        };
        self.push_occurrence(field.name_span, &symbol);
        self.field_defs.insert(target, symbol);
    }

    fn define_method_site(&mut self, target: MethodTarget, function: &Function) {
        let symbol = SymbolData {
            key: SymbolKey::Method(target),
            kind: SymbolKind::Method,
            name: function.name.clone(),
            detail: render_function_signature(self.module, function),
            ty: None,
            definition_span: Some(function.name_span),
        };
        self.push_occurrence(function.name_span, &symbol);
        self.method_defs.insert(target, symbol);
    }

    fn define_param(
        &mut self,
        binding: ParamBinding,
        name: String,
        span: Span,
        detail: String,
        ty: Option<String>,
    ) {
        let symbol = SymbolData {
            key: SymbolKey::Param(binding),
            kind: SymbolKind::Parameter,
            name,
            detail,
            ty,
            definition_span: Some(span),
        };
        self.push_occurrence(span, &symbol);
        self.param_defs.insert(binding, symbol);
    }

    fn define_self(
        &mut self,
        scope: ScopeId,
        kind: ReceiverKind,
        span: Span,
        receiver_ty: Option<String>,
    ) {
        if self.self_defs.contains_key(&scope) {
            return;
        }

        let detail = match receiver_ty.clone() {
            Some(receiver_ty) => format!("receiver {}: {}", render_receiver(kind), receiver_ty),
            None => format!("receiver {}", render_receiver(kind)),
        };
        let symbol = SymbolData {
            key: SymbolKey::SelfValue(scope),
            kind: SymbolKind::SelfParameter,
            name: "self".to_owned(),
            detail,
            ty: receiver_ty,
            definition_span: Some(span),
        };
        self.push_occurrence(span, &symbol);
        self.self_defs.insert(scope, symbol);
    }

    fn define_local(&mut self, local_id: LocalId) {
        if self.local_defs.contains_key(&local_id) {
            return;
        }

        let local = self.module.local(local_id);
        let ty = self.typeck.local_ty(local_id).map(ToString::to_string);
        let detail = match &ty {
            Some(ty) => format!("local {}: {}", local.name, ty),
            None => format!("local {}", local.name),
        };
        let symbol = SymbolData {
            key: SymbolKey::Local(local_id),
            kind: SymbolKind::Local,
            name: local.name.clone(),
            detail,
            ty,
            definition_span: Some(local.span),
        };
        self.push_occurrence(local.span, &symbol);
        self.local_defs.insert(local_id, symbol);
    }

    fn define_import(&mut self, binding: ImportBinding) {
        if self.import_defs.contains_key(&binding) {
            return;
        }

        let symbol = Self::import_symbol(&binding);
        self.push_occurrence(binding.definition_span, &symbol);
        self.import_defs.insert(binding, symbol);
    }

    fn symbol_for_value_resolution(
        &self,
        resolution: &ValueResolution,
        scope: Option<ScopeId>,
    ) -> Option<SymbolData> {
        match resolution {
            ValueResolution::Local(local_id) => self.local_defs.get(local_id).cloned(),
            ValueResolution::Param(binding) => self.param_defs.get(binding).cloned(),
            ValueResolution::SelfValue => self.lookup_self(scope),
            ValueResolution::Function(function_ref) => {
                self.function_defs.get(function_ref).cloned()
            }
            ValueResolution::Item(item_id) => self.item_defs.get(item_id).cloned(),
            ValueResolution::Import(binding) => self.import_defs.get(binding).cloned(),
        }
    }

    fn symbol_for_type_resolution(&self, resolution: &TypeResolution) -> Option<SymbolData> {
        match resolution {
            TypeResolution::Generic(binding) => self.generic_defs.get(binding).cloned(),
            TypeResolution::Builtin(builtin) => Some(SymbolData {
                key: SymbolKey::BuiltinType(*builtin),
                kind: SymbolKind::BuiltinType,
                name: builtin_type_name(*builtin).to_owned(),
                detail: format!("builtin type {}", builtin_type_name(*builtin)),
                ty: None,
                definition_span: None,
            }),
            TypeResolution::Item(item_id) => self.item_defs.get(item_id).cloned(),
            TypeResolution::Import(binding) => self.import_defs.get(binding).cloned(),
        }
    }

    fn import_symbol(binding: &ImportBinding) -> SymbolData {
        SymbolData {
            key: SymbolKey::Import(binding.clone()),
            kind: SymbolKind::Import,
            name: binding.local_name.clone(),
            detail: format!("import {}", render_path(&binding.path)),
            ty: None,
            definition_span: Some(binding.definition_span),
        }
    }

    fn record_completion_site(
        &mut self,
        span: Span,
        scope: ScopeId,
        namespace: CompletionNamespace,
    ) {
        if span.is_empty() {
            return;
        }

        self.ensure_completion_scope(scope);
        let root_scope = self.root_scope(scope);
        self.module_completion_scope.get_or_insert(root_scope);
        self.completion_sites.push(CompletionSite {
            span,
            scope,
            namespace,
        });
    }

    fn ensure_completion_scope(&mut self, scope_id: ScopeId) {
        if self.completion_scopes.contains_key(&scope_id) {
            return;
        }

        let (parent, value_bindings, type_bindings) = {
            let scope = self.resolution.scopes.scope(scope_id);
            (
                scope.parent,
                scope.value_bindings.clone(),
                scope.type_bindings.clone(),
            )
        };
        if let Some(parent_id) = parent {
            self.ensure_completion_scope(parent_id);
        }

        let value_bindings = value_bindings
            .into_iter()
            .filter_map(|binding| {
                self.symbol_for_value_resolution(&binding.resolution, Some(scope_id))
            })
            .collect();
        let type_bindings = type_bindings
            .into_iter()
            .filter_map(|binding| self.symbol_for_type_resolution(&binding.resolution))
            .collect();

        self.completion_scopes.insert(
            scope_id,
            CompletionScope {
                parent,
                value_bindings,
                type_bindings,
            },
        );
    }

    fn root_scope(&self, scope_id: ScopeId) -> ScopeId {
        let mut next = scope_id;
        while let Some(parent) = self.resolution.scopes.scope(next).parent {
            next = parent;
        }
        next
    }

    fn record_member_completion_site(&mut self, span: Span, object: ExprId) {
        let items = self.member_completion_items_for_object(object);
        self.record_semantic_completion_site(span, items);
    }

    fn member_completion_items_for_object(&self, object: ExprId) -> Vec<CompletionItem> {
        if let Some(object_ty) = self.typeck.expr_ty(object) {
            let items = self.member_completion_items(object_ty);
            if !items.is_empty() {
                return items;
            }
        }

        if matches!(self.module.expr(object).kind, ExprKind::Name(_))
            && let Some(resolution) = self.resolution.expr_resolution(object)
            && let Some(item_id) = self.enum_item_for_value_resolution(resolution)
        {
            return self.variant_completion_items(item_id);
        }

        Vec::new()
    }

    fn record_variant_path_completion_site(&mut self, path: &Path, item_id: ItemId) {
        if path.segments.len() != 2 {
            return;
        }
        let Some(span) = path.last_segment_span() else {
            return;
        };
        let items = self.variant_completion_items(item_id);
        self.record_semantic_completion_site(span, items);
    }

    fn record_semantic_completion_site(&mut self, span: Span, items: Vec<CompletionItem>) {
        if span.is_empty() || items.is_empty() {
            return;
        }

        self.semantic_completion_sites
            .push(SemanticCompletionSite { span, items });
    }

    fn enum_item_for_value_resolution(&self, resolution: &ValueResolution) -> Option<ItemId> {
        match resolution {
            ValueResolution::Item(item_id)
                if matches!(self.module.item(*item_id).kind, ItemKind::Enum(_)) =>
            {
                Some(*item_id)
            }
            ValueResolution::Import(binding) => self.local_enum_item_for_import_binding(binding),
            _ => None,
        }
    }

    fn enum_item_for_type_resolution(&self, resolution: &TypeResolution) -> Option<ItemId> {
        match resolution {
            TypeResolution::Item(item_id)
                if matches!(self.module.item(*item_id).kind, ItemKind::Enum(_)) =>
            {
                Some(*item_id)
            }
            TypeResolution::Import(binding) => self.local_enum_item_for_import_binding(binding),
            _ => None,
        }
    }

    fn struct_item_for_value_resolution(&self, resolution: &ValueResolution) -> Option<ItemId> {
        match resolution {
            ValueResolution::Item(item_id)
                if matches!(self.module.item(*item_id).kind, ItemKind::Struct(_)) =>
            {
                Some(*item_id)
            }
            ValueResolution::Import(binding) => self.local_struct_item_for_import_binding(binding),
            _ => None,
        }
    }

    fn struct_item_for_root_value_path(
        &self,
        path: &Path,
        resolution: &ValueResolution,
    ) -> Option<ItemId> {
        if path.segments.len() != 1 {
            return None;
        }
        self.struct_item_for_value_resolution(resolution)
    }

    fn struct_item_for_type_resolution(&self, resolution: &TypeResolution) -> Option<ItemId> {
        match resolution {
            TypeResolution::Item(item_id)
                if matches!(self.module.item(*item_id).kind, ItemKind::Struct(_)) =>
            {
                Some(*item_id)
            }
            TypeResolution::Import(binding) => self.local_struct_item_for_import_binding(binding),
            _ => None,
        }
    }

    fn struct_item_for_root_type_path(
        &self,
        path: &Path,
        resolution: &TypeResolution,
    ) -> Option<ItemId> {
        if path.segments.len() != 1 {
            return None;
        }
        self.struct_item_for_type_resolution(resolution)
    }

    fn local_enum_item_for_import_binding(&self, binding: &ImportBinding) -> Option<ItemId> {
        let [name] = binding.path.segments.as_slice() else {
            return None;
        };

        self.module.items.iter().copied().find(|item_id| {
            matches!(
                &self.module.item(*item_id).kind,
                ItemKind::Enum(enum_decl) if enum_decl.name == *name
            )
        })
    }

    fn local_struct_item_for_import_binding(&self, binding: &ImportBinding) -> Option<ItemId> {
        let [name] = binding.path.segments.as_slice() else {
            return None;
        };

        self.module.items.iter().copied().find(|item_id| {
            matches!(
                &self.module.item(*item_id).kind,
                ItemKind::Struct(struct_decl) if struct_decl.name == *name
            )
        })
    }

    fn member_completion_items(&self, object_ty: &ql_typeck::Ty) -> Vec<CompletionItem> {
        let ql_typeck::Ty::Item { item_id, .. } = object_ty else {
            return Vec::new();
        };

        let mut items = Vec::new();
        let mut seen = HashSet::new();

        self.collect_method_completion_items(
            object_ty,
            MemberCompletionSource::Impl,
            &mut seen,
            &mut items,
        );
        self.collect_method_completion_items(
            object_ty,
            MemberCompletionSource::Extend,
            &mut seen,
            &mut items,
        );

        if let ItemKind::Struct(struct_decl) = &self.module.item(*item_id).kind {
            for (field_index, field) in struct_decl.fields.iter().enumerate() {
                if !seen.insert(field.name.clone()) {
                    continue;
                }

                let target = FieldTarget {
                    item_id: *item_id,
                    field_index,
                };
                if let Some(symbol) = self.field_defs.get(&target) {
                    items.push(completion_item_from_symbol(symbol));
                }
            }
        }

        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        items
    }

    fn variant_completion_items(&self, item_id: ItemId) -> Vec<CompletionItem> {
        let ItemKind::Enum(enum_decl) = &self.module.item(item_id).kind else {
            return Vec::new();
        };

        let mut items = Vec::new();
        let mut seen = HashSet::new();

        for (variant_index, variant) in enum_decl.variants.iter().enumerate() {
            if !seen.insert(variant.name.clone()) {
                continue;
            }

            let target = VariantTarget {
                item_id,
                variant_index,
            };
            if let Some(symbol) = self.variant_defs.get(&target) {
                items.push(completion_item_from_symbol(symbol));
            }
        }

        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        items
    }

    fn collect_method_completion_items(
        &self,
        object_ty: &ql_typeck::Ty,
        source: MemberCompletionSource,
        seen: &mut HashSet<String>,
        items: &mut Vec<CompletionItem>,
    ) {
        let mut candidates: HashMap<String, Vec<MethodTarget>> = HashMap::new();

        for &candidate_item_id in &self.module.items {
            match &self.module.item(candidate_item_id).kind {
                ItemKind::Impl(impl_block) if source == MemberCompletionSource::Impl => {
                    let target_ty =
                        ql_typeck::lower_type(self.module, self.resolution, impl_block.target);
                    if object_ty.compatible_with(&target_ty) {
                        for (method_index, method) in impl_block.methods.iter().enumerate() {
                            candidates
                                .entry(method.name.clone())
                                .or_default()
                                .push(MethodTarget {
                                    item_id: candidate_item_id,
                                    method_index,
                                });
                        }
                    }
                }
                ItemKind::Extend(extend_block) if source == MemberCompletionSource::Extend => {
                    let target_ty =
                        ql_typeck::lower_type(self.module, self.resolution, extend_block.target);
                    if object_ty.compatible_with(&target_ty) {
                        for (method_index, method) in extend_block.methods.iter().enumerate() {
                            candidates
                                .entry(method.name.clone())
                                .or_default()
                                .push(MethodTarget {
                                    item_id: candidate_item_id,
                                    method_index,
                                });
                        }
                    }
                }
                _ => {}
            }
        }

        let mut names = candidates.into_iter().collect::<Vec<_>>();
        names.sort_by(|left, right| left.0.cmp(&right.0));
        for (name, targets) in names {
            if targets.len() != 1 || !seen.insert(name.clone()) {
                continue;
            }
            if let Some(symbol) = self.method_defs.get(&targets[0]) {
                items.push(completion_item_from_symbol(symbol));
            }
        }
    }

    fn record_field_shorthand_occurrence(&mut self, target: FieldTarget, span: Span) {
        let Some(binding_text) = self.source.get(span.start..span.end) else {
            return;
        };

        self.field_shorthand_occurrences
            .entry(target)
            .or_default()
            .push(FieldShorthandOccurrence {
                span,
                binding_text: binding_text.to_owned(),
            });
    }

    fn record_binding_shorthand_occurrence(&mut self, key: SymbolKey, span: Span) {
        let Some(label_text) = self.source.get(span.start..span.end) else {
            return;
        };

        self.binding_shorthand_occurrences
            .entry(key)
            .or_default()
            .push(BindingShorthandOccurrence {
                span,
                label_text: label_text.to_owned(),
            });
    }

    fn symbol_for_member_target(&self, target: MemberTarget) -> Option<SymbolData> {
        match target {
            MemberTarget::Field(target) => self.field_defs.get(&target).cloned(),
            MemberTarget::Method(target) => self.method_defs.get(&target).cloned(),
        }
    }

    fn symbol_for_variant_target(&self, target: VariantTarget) -> Option<SymbolData> {
        self.variant_defs.get(&target).cloned()
    }

    fn field_target_for_struct_item(
        &self,
        item_id: ItemId,
        field_name: &str,
    ) -> Option<FieldTarget> {
        let ItemKind::Struct(struct_decl) = &self.module.item(item_id).kind else {
            return None;
        };
        struct_decl
            .fields
            .iter()
            .enumerate()
            .find(|(_, field)| field.name == field_name)
            .map(|(field_index, _)| FieldTarget {
                item_id,
                field_index,
            })
    }

    fn variant_target_for_enum_item(
        &self,
        item_id: ItemId,
        variant_name: &str,
    ) -> Option<VariantTarget> {
        let ItemKind::Enum(enum_decl) = &self.module.item(item_id).kind else {
            return None;
        };
        enum_decl
            .variants
            .iter()
            .enumerate()
            .find(|(_, variant)| variant.name == variant_name)
            .map(|(variant_index, _)| VariantTarget {
                item_id,
                variant_index,
            })
    }

    fn lookup_self(&self, scope: Option<ScopeId>) -> Option<SymbolData> {
        let mut next = scope;
        while let Some(scope_id) = next {
            if let Some(symbol) = self.self_defs.get(&scope_id) {
                return Some(symbol.clone());
            }
            next = self.resolution.scopes.scope(scope_id).parent;
        }
        None
    }

    fn push_occurrence(&mut self, span: Span, symbol: &SymbolData) {
        if span.is_empty() {
            return;
        }

        self.occurrences.push(IndexedSymbol {
            span,
            key: symbol.key.clone(),
            hover: HoverInfo {
                span,
                kind: symbol.kind,
                name: symbol.name.clone(),
                detail: symbol.detail.clone(),
                ty: symbol.ty.clone(),
                definition_span: symbol.definition_span,
            },
        });
    }

    fn index_variant_value_path_use(&mut self, path: &Path, resolution: &ValueResolution) {
        let Some(item_id) = self.enum_item_for_value_resolution(resolution) else {
            return;
        };
        if path.segments.len() != 2 {
            return;
        }
        let Some(variant_name) = path.segments.last() else {
            return;
        };
        let Some(variant_span) = path.last_segment_span() else {
            return;
        };
        if let Some(target) = self.variant_target_for_enum_item(item_id, variant_name)
            && let Some(symbol) = self.symbol_for_variant_target(target)
        {
            self.push_occurrence(variant_span, &symbol);
        }
    }

    fn index_variant_type_path_use(&mut self, path: &Path, resolution: &TypeResolution) {
        let Some(item_id) = self.enum_item_for_type_resolution(resolution) else {
            return;
        };
        if path.segments.len() != 2 {
            return;
        }
        let Some(variant_name) = path.segments.last() else {
            return;
        };
        let Some(variant_span) = path.last_segment_span() else {
            return;
        };
        if let Some(target) = self.variant_target_for_enum_item(item_id, variant_name)
            && let Some(symbol) = self.symbol_for_variant_target(target)
        {
            self.push_occurrence(variant_span, &symbol);
        }
    }

    fn index_variant_member_use(&mut self, item_id: ItemId, variant_name: &str, span: Span) {
        if let Some(target) = self.variant_target_for_enum_item(item_id, variant_name)
            && let Some(symbol) = self.symbol_for_variant_target(target)
        {
            self.push_occurrence(span, &symbol);
        }
    }

    fn path_root_span(&self, path: &Path, fallback: Span) -> Span {
        path.first_segment_span()
            .filter(|span| !span.is_empty())
            .unwrap_or_else(|| self.root_span(fallback))
    }

    fn root_span(&self, span: Span) -> Span {
        let Some(slice) = self.source.get(span.start..span.end) else {
            return span;
        };

        for (offset, ch) in slice.char_indices() {
            if matches!(ch, '.' | '[' | '{' | '(' | ' ' | '\t' | '\r' | '\n') {
                if offset == 0 {
                    return span;
                }
                return Span::new(span.start, span.start + offset);
            }
        }

        span
    }
}

fn completion_span_contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn skip_whitespace_prefix(text: &str, start: usize) -> usize {
    let mut offset = start;
    while let Some(ch) = text.get(offset..).and_then(|suffix| suffix.chars().next()) {
        if !ch.is_whitespace() {
            break;
        }
        offset += ch.len_utf8();
    }
    offset
}

const fn completion_namespace_rank(namespace: CompletionNamespace) -> usize {
    match namespace {
        CompletionNamespace::Value => 0,
        CompletionNamespace::Type => 1,
    }
}

fn render_function_signature(module: &Module, function: &Function) -> String {
    let mut parts = Vec::new();
    if function.is_async {
        parts.push("async".to_owned());
    }
    if function.is_unsafe {
        parts.push("unsafe".to_owned());
    }
    if let Some(abi) = &function.abi {
        parts.push(format!("extern \"{}\"", abi));
    }

    let generics = render_generics(&function.generics);
    let params = function
        .params
        .iter()
        .map(|param| render_param(module, param))
        .collect::<Vec<_>>()
        .join(", ");
    let mut signature = format!("fn {}{}({})", function.name, generics, params);
    if let Some(return_type) = function.return_type {
        signature.push_str(&format!(" -> {}", render_type(module, return_type)));
    }
    if !function.where_clause.is_empty() {
        signature.push_str(&render_where_clause(module, &function.where_clause));
    }

    parts.push(signature);
    parts.join(" ")
}

fn render_variant_detail(module: &Module, enum_name: &str, variant: &EnumVariant) -> String {
    match &variant.fields {
        VariantFields::Unit => format!("variant {}.{}", enum_name, variant.name),
        VariantFields::Tuple(items) => format!(
            "variant {}.{}({})",
            enum_name,
            variant.name,
            items
                .iter()
                .map(|type_id| render_type(module, *type_id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        VariantFields::Struct(fields) => format!(
            "variant {}.{} {{ {} }}",
            enum_name,
            variant.name,
            fields
                .iter()
                .map(|field| format!("{}: {}", field.name, render_type(module, field.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_struct_detail(is_data: bool, name: &str, generics: &[GenericParam]) -> String {
    let keyword = if is_data { "data struct" } else { "struct" };
    format!("{} {}{}", keyword, name, render_generics(generics))
}

fn render_type_alias_detail(module: &Module, alias: &TypeAlias) -> String {
    let keyword = if alias.is_opaque {
        "opaque type"
    } else {
        "type"
    };
    format!(
        "{} {}{} = {}",
        keyword,
        alias.name,
        render_generics(&alias.generics),
        render_type(module, alias.ty)
    )
}

fn render_generic_detail(generic: &GenericParam) -> String {
    if generic.bounds.is_empty() {
        format!("generic {}", generic.name)
    } else {
        format!(
            "generic {}: {}",
            generic.name,
            generic
                .bounds
                .iter()
                .map(render_path)
                .collect::<Vec<_>>()
                .join(" + ")
        )
    }
}

fn render_generics(generics: &[GenericParam]) -> String {
    if generics.is_empty() {
        return String::new();
    }

    format!(
        "[{}]",
        generics
            .iter()
            .map(|generic| {
                if generic.bounds.is_empty() {
                    generic.name.clone()
                } else {
                    format!(
                        "{}: {}",
                        generic.name,
                        generic
                            .bounds
                            .iter()
                            .map(render_path)
                            .collect::<Vec<_>>()
                            .join(" + ")
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_param(module: &Module, param: &Param) -> String {
    match param {
        Param::Regular(param) => format!("{}: {}", param.name, render_type(module, param.ty)),
        Param::Receiver(receiver) => render_receiver(receiver.kind).to_owned(),
    }
}

fn render_receiver(kind: ReceiverKind) -> &'static str {
    match kind {
        ReceiverKind::ReadOnly => "self",
        ReceiverKind::Mutable => "var self",
        ReceiverKind::Move => "move self",
    }
}

fn render_where_clause(module: &Module, predicates: &[WherePredicate]) -> String {
    format!(
        " where {}",
        predicates
            .iter()
            .map(|predicate| {
                format!(
                    "{}: {}",
                    render_type(module, predicate.target),
                    predicate
                        .bounds
                        .iter()
                        .map(render_path)
                        .collect::<Vec<_>>()
                        .join(" + ")
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_type(module: &Module, type_id: TypeId) -> String {
    match &module.ty(type_id).kind {
        TypeKind::Pointer { is_const, inner } => {
            let qualifier = if *is_const { "const" } else { "mut" };
            format!("*{} {}", qualifier, render_type(module, *inner))
        }
        TypeKind::Array { element, len } => {
            format!("[{}; {}]", render_type(module, *element), len)
        }
        TypeKind::Named { path, args } => {
            if args.is_empty() {
                render_path(path)
            } else {
                format!(
                    "{}[{}]",
                    render_path(path),
                    args.iter()
                        .map(|type_id| render_type(module, *type_id))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        TypeKind::Tuple(items) => match items.as_slice() {
            [] => "()".to_owned(),
            [item] => format!("({},)", render_type(module, *item)),
            _ => format!(
                "({})",
                items
                    .iter()
                    .map(|type_id| render_type(module, *type_id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        TypeKind::Callable { params, ret } => format!(
            "fn({}) -> {}",
            params
                .iter()
                .map(|type_id| render_type(module, *type_id))
                .collect::<Vec<_>>()
                .join(", "),
            render_type(module, *ret)
        ),
    }
}

fn render_path(path: &Path) -> String {
    path.segments.join(".")
}

fn builtin_type_name(builtin: BuiltinType) -> &'static str {
    match builtin {
        BuiltinType::Bool => "Bool",
        BuiltinType::Char => "Char",
        BuiltinType::String => "String",
        BuiltinType::Bytes => "Bytes",
        BuiltinType::Void => "Void",
        BuiltinType::Never => "Never",
        BuiltinType::Int => "Int",
        BuiltinType::UInt => "UInt",
        BuiltinType::I8 => "I8",
        BuiltinType::I16 => "I16",
        BuiltinType::I32 => "I32",
        BuiltinType::I64 => "I64",
        BuiltinType::ISize => "ISize",
        BuiltinType::U8 => "U8",
        BuiltinType::U16 => "U16",
        BuiltinType::U32 => "U32",
        BuiltinType::U64 => "U64",
        BuiltinType::USize => "USize",
        BuiltinType::F32 => "F32",
        BuiltinType::F64 => "F64",
    }
}

fn completion_item_from_symbol(symbol: &SymbolData) -> CompletionItem {
    CompletionItem {
        label: symbol.name.clone(),
        insert_text: completion_insert_text(&symbol.name),
        kind: symbol.kind,
        detail: symbol.detail.clone(),
        ty: symbol.ty.clone(),
    }
}

fn completion_insert_text(name: &str) -> String {
    if is_keyword(name) {
        format!("`{name}`")
    } else {
        name.to_owned()
    }
}

fn validate_rename_text(text: &str) -> Result<(), RenameError> {
    let escaped = text
        .strip_prefix('`')
        .and_then(|value| value.strip_suffix('`'));
    if let Some(inner) = escaped {
        return is_valid_identifier(inner)
            .then_some(())
            .ok_or_else(|| RenameError::InvalidIdentifier(text.to_owned()));
    }

    if !is_valid_identifier(text) {
        return Err(RenameError::InvalidIdentifier(text.to_owned()));
    }
    if is_keyword(text) {
        return Err(RenameError::Keyword(text.to_owned()));
    }

    Ok(())
}
