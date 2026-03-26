use std::{collections::HashMap, fmt};

use ql_ast::{Path, ReceiverKind};
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
        builder.finish()
    }

    pub(crate) fn symbol_at(&self, offset: usize) -> Option<HoverInfo> {
        self.occurrences
            .iter()
            .find(|entry| entry.span.contains(offset))
            .map(|entry| entry.hover.clone())
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
        }
    }

    fn finish(mut self) -> QueryIndex {
        self.occurrences
            .sort_by_key(|entry| (entry.span.len(), entry.span.start, entry.span.end));
        for occurrences in self.field_shorthand_occurrences.values_mut() {
            occurrences.sort_by_key(|occurrence| (occurrence.span.start, occurrence.span.end));
            occurrences.dedup_by(|left, right| left.span == right.span);
        }
        QueryIndex {
            occurrences: self.occurrences,
            field_shorthand_occurrences: self.field_shorthand_occurrences,
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
                        let field_owner = match self.resolution.pattern_resolution(pattern_id) {
                            Some(ValueResolution::Item(item_id)) => Some(*item_id),
                            _ => None,
                        };
                        for field in fields {
                            if let Some(item_id) = field_owner
                                && let Some(target) =
                                    self.field_target_for_struct_item(item_id, &field.name)
                            {
                                if field.is_shorthand {
                                    self.record_field_shorthand_occurrence(target, field.name_span);
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
                } else if let Some(ValueResolution::Item(item_id)) =
                    self.resolution.expr_resolution(expr_id)
                {
                    self.index_variant_member_use(*item_id, field, *field_span);
                }
            }
            ExprKind::Bracket { target, items } => {
                self.index_expr_use(*target);
                for &item in items {
                    self.index_expr_use(item);
                }
            }
            ExprKind::StructLiteral { path, fields } => {
                let field_owner = match self.resolution.struct_literal_resolution(expr_id) {
                    Some(TypeResolution::Item(item_id)) => Some(*item_id),
                    _ => None,
                };
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
        let ValueResolution::Item(item_id) = resolution else {
            return;
        };
        let Some(variant_name) = path.segments.last() else {
            return;
        };
        let Some(variant_span) = path.last_segment_span() else {
            return;
        };
        if path.segments.len() < 2 {
            return;
        }
        if let Some(target) = self.variant_target_for_enum_item(*item_id, variant_name)
            && let Some(symbol) = self.symbol_for_variant_target(target)
        {
            self.push_occurrence(variant_span, &symbol);
        }
    }

    fn index_variant_type_path_use(&mut self, path: &Path, resolution: &TypeResolution) {
        let TypeResolution::Item(item_id) = resolution else {
            return;
        };
        let Some(variant_name) = path.segments.last() else {
            return;
        };
        let Some(variant_span) = path.last_segment_span() else {
            return;
        };
        if path.segments.len() < 2 {
            return;
        }
        if let Some(target) = self.variant_target_for_enum_item(*item_id, variant_name)
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
