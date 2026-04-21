mod query;
mod runtime;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use ql_ast::{Item as AstItem, ItemKind as AstItemKind, Visibility as AstVisibility};
use ql_borrowck::{
    BorrowckResult, analyze_module as analyze_borrowck, render_result as render_borrowck_result,
};
use ql_diagnostics::{Diagnostic, Label, render_diagnostics};
use ql_hir::{ExprId, ItemId, ItemKind as HirItemKind, LocalId, PatternId, lower_module};
use ql_lexer::{Token, TokenKind, is_keyword, is_valid_identifier, lex};
use ql_mir::{
    MirModule, lower_module_with_typeck as lower_mir_with_typeck,
    render_module as render_mir_module,
};
use ql_parser::{ParseError, parse_source};
use ql_project::{
    InterfaceArtifact, InterfaceError, InterfaceModule, ProjectError, ProjectManifest,
    collect_package_sources, default_interface_path, load_interface_artifact,
    load_project_manifest, load_reference_manifests,
};
use ql_resolve::{ImportBinding, ResolutionMap, TypeResolution, resolve_module};
use ql_span::Span;
use ql_typeck::{Ty, TypeckResult, analyze_module as analyze_types};
use query::QueryIndex;
pub use query::{
    AsyncContextInfo, AsyncOperatorKind, CompletionItem, DefinitionTarget, DocumentSymbolTarget,
    HoverInfo, LoopControlContextInfo, LoopControlKind, ReferenceTarget, RenameEdit, RenameError,
    RenameResult, RenameTarget, SemanticTokenOccurrence, SymbolKind,
};
pub use runtime::RuntimeRequirement;

/// Parsed-and-lowered semantic analysis snapshot shared by CLI and future LSP work.
#[derive(Clone, Debug)]
pub struct Analysis {
    ast: ql_ast::Module,
    hir: ql_hir::Module,
    mir: MirModule,
    resolution: ResolutionMap,
    typeck: TypeckResult,
    borrowck: BorrowckResult,
    index: QueryIndex,
    runtime_requirements: Vec<RuntimeRequirement>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct PackageModuleAnalysis {
    path: PathBuf,
    analysis: Analysis,
}

#[derive(Clone, Debug)]
pub struct DependencyInterface {
    manifest: ProjectManifest,
    interface_path: PathBuf,
    artifact: InterfaceArtifact,
    symbols: Vec<DependencySymbol>,
}

#[derive(Clone, Debug)]
pub struct PackageAnalysis {
    manifest: ProjectManifest,
    modules: Vec<PackageModuleAnalysis>,
    dependencies: Vec<DependencyInterface>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencySymbol {
    pub package_name: String,
    pub source_path: String,
    pub kind: SymbolKind,
    pub name: String,
    pub detail: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplementationTarget {
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyHoverInfo {
    pub span: Span,
    pub package_name: String,
    pub source_path: String,
    pub kind: SymbolKind,
    pub name: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyDefinitionTarget {
    pub package_name: String,
    pub manifest_path: PathBuf,
    pub source_path: String,
    pub kind: SymbolKind,
    pub name: String,
    pub path: PathBuf,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyResolvedTarget {
    pub import_span: Span,
    pub package_name: String,
    pub manifest_path: PathBuf,
    pub source_path: String,
    pub kind: SymbolKind,
    pub name: String,
    pub detail: String,
    pub path: PathBuf,
    pub definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyEnumCompletionTarget {
    pub package_name: String,
    pub manifest_path: PathBuf,
    pub source_path: String,
    pub enum_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyStructCompletionTarget {
    pub package_name: String,
    pub manifest_path: PathBuf,
    pub source_path: String,
    pub struct_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyStructFieldCompletionTarget {
    pub target: DependencyStructCompletionTarget,
    pub excluded_field_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyImportOccurrence {
    local_name: String,
    span: Span,
    is_definition: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceDependencyImportOccurrence {
    local_name: String,
    span: Span,
    is_definition: bool,
    target: DependencyResolvedTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceParameterCandidate {
    name: String,
    span: Span,
    type_root: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceLocalCandidate {
    name: String,
    span: Span,
    rhs_value: Option<BrokenSourceValueCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceValueCandidate {
    root_name: String,
    root_span: Span,
    root_called: bool,
    root_question_unwrap: bool,
    root_indexed_iterable: bool,
    segments: Vec<BrokenSourceValueSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceValueSegment {
    name: String,
    kind: BrokenSourceValueSegmentKind,
    question_unwrap: bool,
    indexed_iterable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BrokenSourceValueSegmentKind {
    Field,
    Method,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceDependencyMemberSite {
    receiver_candidate: BrokenSourceValueCandidate,
    member_span: Span,
    member_name: String,
    member_kind: BrokenSourceValueSegmentKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyValueOccurrence {
    kind: SymbolKind,
    local_name: String,
    reference_span: Span,
    definition_span: Span,
    definition_rename: DependencyValueDefinitionRename,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    path: PathBuf,
    is_definition: bool,
}

#[derive(Debug)]
pub enum PackageAnalysisError {
    Project(ProjectError),
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    SourceDiagnostics {
        path: PathBuf,
        source: String,
        diagnostics: Vec<Diagnostic>,
    },
    InterfaceNotFound {
        package_name: String,
        path: PathBuf,
    },
    InterfaceParse {
        path: PathBuf,
        message: String,
    },
}

impl PackageModuleAnalysis {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn analysis(&self) -> &Analysis {
        &self.analysis
    }
}

impl DependencyInterface {
    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn interface_path(&self) -> &Path {
        &self.interface_path
    }

    pub fn artifact(&self) -> &InterfaceArtifact {
        &self.artifact
    }

    pub fn symbols(&self) -> &[DependencySymbol] {
        &self.symbols
    }

    pub fn workspace_symbols(&self) -> Vec<DependencySymbol> {
        let mut symbols = self.symbols.clone();
        for module in &self.artifact.modules {
            for item in &module.syntax.items {
                let AstItemKind::Enum(enum_decl) = &item.kind else {
                    continue;
                };
                if !is_public(&enum_decl.visibility) {
                    continue;
                }
                for variant in &enum_decl.variants {
                    push_dependency_symbol(
                        &self.artifact.package_name,
                        &module.source_path,
                        SymbolKind::Variant,
                        &variant.name,
                        variant.name_span,
                        dependency_variant_detail(&enum_decl.name, variant),
                        &mut symbols,
                    );
                }
            }
        }
        symbols
    }

    pub fn symbols_named(&self, name: &str) -> Vec<&DependencySymbol> {
        self.symbols
            .iter()
            .filter(|symbol| symbol.name == name)
            .collect()
    }

    pub fn definition_span_for_symbol(&self, symbol: &DependencySymbol) -> Option<Span> {
        self.artifact_span_for(symbol)
    }

    fn public_type_target_for_type_expr(
        &self,
        ty: &ql_ast::TypeExpr,
    ) -> Option<DependencyDefinitionTarget> {
        let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
            return None;
        };
        let [type_name] = path.segments.as_slice() else {
            return None;
        };
        let mut matches = self
            .symbols_named(type_name)
            .into_iter()
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    SymbolKind::Struct
                        | SymbolKind::Enum
                        | SymbolKind::Trait
                        | SymbolKind::TypeAlias
                )
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return None;
        }
        let symbol = matches.pop()?;
        let span = self.artifact_span_for(symbol)?;
        Some(DependencyDefinitionTarget {
            package_name: symbol.package_name.clone(),
            manifest_path: self.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            path: self.interface_path.clone(),
            span,
        })
    }

    fn public_question_inner_type_target_for_type_expr(
        &self,
        ty: &ql_ast::TypeExpr,
    ) -> Option<DependencyDefinitionTarget> {
        let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
            return None;
        };
        let [type_name] = path.segments.as_slice() else {
            return None;
        };
        let inner = match (type_name.as_str(), args.as_slice()) {
            ("Option", [inner]) => inner,
            ("Result", [inner, ..]) => inner,
            _ => return None,
        };
        self.public_type_target_for_type_expr(inner)
            .or_else(|| self.public_question_inner_type_target_for_type_expr(inner))
    }

    fn public_question_inner_iterable_element_type_target_for_type_expr(
        &self,
        ty: &ql_ast::TypeExpr,
    ) -> Option<DependencyDefinitionTarget> {
        let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
            return None;
        };
        let [type_name] = path.segments.as_slice() else {
            return None;
        };
        let inner = match (type_name.as_str(), args.as_slice()) {
            ("Option", [inner]) => inner,
            ("Result", [inner, ..]) => inner,
            _ => return None,
        };
        self.public_iterable_element_type_target_for_type_expr(inner)
            .or_else(|| {
                self.public_question_inner_iterable_element_type_target_for_type_expr(inner)
            })
    }

    fn public_common_type_target_for_type_exprs(
        &self,
        items: &[ql_ast::TypeExpr],
    ) -> Option<DependencyDefinitionTarget> {
        let mut items = items.iter();
        let first = self.public_type_target_for_type_expr(items.next()?)?;
        for item in items {
            let target = self.public_type_target_for_type_expr(item)?;
            if target != first {
                return None;
            }
        }
        Some(first)
    }

    fn public_iterable_element_type_target_for_type_expr(
        &self,
        ty: &ql_ast::TypeExpr,
    ) -> Option<DependencyDefinitionTarget> {
        match &ty.kind {
            ql_ast::TypeExprKind::Array { element, .. } => {
                self.public_type_target_for_type_expr(element)
            }
            ql_ast::TypeExprKind::Tuple(items) => {
                self.public_common_type_target_for_type_exprs(items)
            }
            _ => None,
        }
    }

    fn import_path_variants(&self) -> Vec<Vec<String>> {
        let mut variants = self
            .artifact
            .modules
            .iter()
            .filter_map(|module| {
                module
                    .syntax
                    .package
                    .as_ref()
                    .map(|package| package.path.segments.clone())
            })
            .collect::<Vec<_>>();

        if let Some(package) = &self.manifest.package {
            variants.push(vec![package.name.clone()]);
        }

        if variants.is_empty() {
            variants.push(vec![self.artifact.package_name.clone()]);
        }

        variants.sort();
        variants.dedup();
        variants
    }

    fn artifact_span_for(&self, symbol: &DependencySymbol) -> Option<Span> {
        self.artifact_source_span(&symbol.source_path, symbol.span)
    }

    fn artifact_source_span(&self, source_path: &str, span: Span) -> Option<Span> {
        let source = fs::read_to_string(&self.interface_path)
            .ok()?
            .replace("\r\n", "\n");
        let mut search_start = 0usize;
        for module in &self.artifact.modules {
            let header = format!("// source: {}", module.source_path);
            let header_index = source.get(search_start..)?.find(&header)? + search_start;
            let body_search_start = header_index + header.len();
            let module_index = source
                .get(body_search_start..)?
                .find(&module.contents)
                .map(|offset| body_search_start + offset)?;
            if module.source_path == source_path {
                return Some(Span::new(
                    module_index + span.start,
                    module_index + span.end,
                ));
            }
            search_start = module_index + module.contents.len();
        }
        None
    }

    fn variant_completions_for(&self, symbol: &DependencySymbol) -> Option<Vec<CompletionItem>> {
        if symbol.kind != SymbolKind::Enum {
            return None;
        }

        let enum_decl = self.enum_decl_for(symbol)?;

        let mut items = enum_decl
            .variants
            .iter()
            .map(|variant| CompletionItem {
                label: variant.name.clone(),
                insert_text: variant.name.clone(),
                kind: SymbolKind::Variant,
                detail: dependency_variant_detail(&enum_decl.name, variant),
                ty: Some(enum_decl.name.clone()),
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    fn variant_for<'a>(
        &'a self,
        symbol: &DependencySymbol,
        variant_name: &str,
    ) -> Option<&'a ql_ast::EnumVariant> {
        let enum_decl = self.enum_decl_for(symbol)?;
        enum_decl
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
    }

    fn enum_decl_for<'a>(&'a self, symbol: &DependencySymbol) -> Option<&'a ql_ast::EnumDecl> {
        if symbol.kind != SymbolKind::Enum {
            return None;
        }

        self.artifact
            .modules
            .iter()
            .find(|module| module.source_path == symbol.source_path)?
            .syntax
            .items
            .iter()
            .find_map(|item| match &item.kind {
                AstItemKind::Enum(enum_decl)
                    if is_public(&enum_decl.visibility) && enum_decl.name == symbol.name =>
                {
                    Some(enum_decl)
                }
                _ => None,
            })
    }

    fn struct_field_for<'a>(
        &'a self,
        symbol: &DependencySymbol,
        field_name: &str,
    ) -> Option<&'a ql_ast::FieldDecl> {
        let struct_decl = self.struct_decl_for(symbol)?;
        struct_decl
            .fields
            .iter()
            .find(|field| field.name == field_name)
    }

    fn struct_decl_for<'a>(&'a self, symbol: &DependencySymbol) -> Option<&'a ql_ast::StructDecl> {
        if symbol.kind != SymbolKind::Struct {
            return None;
        }

        self.artifact
            .modules
            .iter()
            .find(|module| module.source_path == symbol.source_path)?
            .syntax
            .items
            .iter()
            .find_map(|item| match &item.kind {
                AstItemKind::Struct(struct_decl)
                    if is_public(&struct_decl.visibility) && struct_decl.name == symbol.name =>
                {
                    Some(struct_decl)
                }
                _ => None,
            })
    }

    fn function_decl_for<'a>(
        &'a self,
        symbol: &DependencySymbol,
    ) -> Option<&'a ql_ast::FunctionDecl> {
        if symbol.kind != SymbolKind::Function {
            return None;
        }

        self.artifact
            .modules
            .iter()
            .find(|module| module.source_path == symbol.source_path)?
            .syntax
            .items
            .iter()
            .find_map(|item| match &item.kind {
                AstItemKind::Function(function)
                    if is_public(&function.visibility) && function.name == symbol.name =>
                {
                    Some(function)
                }
                AstItemKind::ExternBlock(extern_block) if is_public(&extern_block.visibility) => {
                    extern_block
                        .functions
                        .iter()
                        .find(|function| function.name == symbol.name)
                }
                _ => None,
            })
    }

    fn function_return_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let function = self.function_decl_for(symbol)?;
        let return_type = function.return_type.as_ref()?;
        self.public_type_target_for_type_expr(return_type)
    }

    fn function_question_return_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let function = self.function_decl_for(symbol)?;
        let return_type = function.return_type.as_ref()?;
        self.public_question_inner_type_target_for_type_expr(return_type)
    }

    fn function_iterable_element_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let function = self.function_decl_for(symbol)?;
        let return_type = function.return_type.as_ref()?;
        self.public_iterable_element_type_target_for_type_expr(return_type)
    }

    fn function_question_iterable_element_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let function = self.function_decl_for(symbol)?;
        let return_type = function.return_type.as_ref()?;
        self.public_question_inner_iterable_element_type_target_for_type_expr(return_type)
    }

    fn global_decl_for<'a>(&'a self, symbol: &DependencySymbol) -> Option<&'a ql_ast::GlobalDecl> {
        if !matches!(symbol.kind, SymbolKind::Const | SymbolKind::Static) {
            return None;
        }

        self.artifact
            .modules
            .iter()
            .find(|module| module.source_path == symbol.source_path)?
            .syntax
            .items
            .iter()
            .find_map(|item| match &item.kind {
                AstItemKind::Const(global)
                    if symbol.kind == SymbolKind::Const
                        && is_public(&global.visibility)
                        && global.name == symbol.name =>
                {
                    Some(global)
                }
                AstItemKind::Static(global)
                    if symbol.kind == SymbolKind::Static
                        && is_public(&global.visibility)
                        && global.name == symbol.name =>
                {
                    Some(global)
                }
                _ => None,
            })
    }

    fn global_type_target(&self, symbol: &DependencySymbol) -> Option<DependencyDefinitionTarget> {
        let global = self.global_decl_for(symbol)?;
        self.public_type_target_for_type_expr(&global.ty)
    }

    fn global_question_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let global = self.global_decl_for(symbol)?;
        self.public_question_inner_type_target_for_type_expr(&global.ty)
    }

    fn global_iterable_element_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let global = self.global_decl_for(symbol)?;
        self.public_iterable_element_type_target_for_type_expr(&global.ty)
    }

    fn global_question_iterable_element_type_target(
        &self,
        symbol: &DependencySymbol,
    ) -> Option<DependencyDefinitionTarget> {
        let global = self.global_decl_for(symbol)?;
        self.public_question_inner_iterable_element_type_target_for_type_expr(&global.ty)
    }

    fn struct_methods_for(
        &self,
        symbol: &DependencySymbol,
    ) -> HashMap<String, DependencyStructResolvedMethod> {
        if symbol.kind != SymbolKind::Struct {
            return HashMap::new();
        }

        let mut impl_candidates: HashMap<String, Vec<DependencyStructResolvedMethod>> =
            HashMap::new();
        let mut extend_candidates: HashMap<String, Vec<DependencyStructResolvedMethod>> =
            HashMap::new();

        for module in &self.artifact.modules {
            for item in &module.syntax.items {
                match &item.kind {
                    AstItemKind::Impl(impl_block)
                        if dependency_type_expr_targets_struct(
                            &impl_block.target,
                            &symbol.name,
                        ) =>
                    {
                        for method in impl_block
                            .methods
                            .iter()
                            .filter(|method| is_public(&method.visibility))
                        {
                            let Some(definition_span) =
                                self.artifact_source_span(&module.source_path, method.name_span)
                            else {
                                continue;
                            };
                            impl_candidates
                                .entry(method.name.clone())
                                .or_default()
                                .push(DependencyStructResolvedMethod {
                                    name: method.name.clone(),
                                    source_path: module.source_path.clone(),
                                    detail: interface_detail_text(
                                        &module.contents,
                                        method.span,
                                        &method.name,
                                    ),
                                    return_type: method
                                        .return_type
                                        .as_ref()
                                        .map(render_dependency_type_expr),
                                    definition_span,
                                    return_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| self.public_type_target_for_type_expr(ty)),
                                    question_return_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| {
                                            self.public_question_inner_type_target_for_type_expr(ty)
                                        }),
                                    iterable_element_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| {
                                            self.public_iterable_element_type_target_for_type_expr(
                                                ty,
                                            )
                                        }),
                                    question_iterable_element_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| {
                                            self.public_question_inner_iterable_element_type_target_for_type_expr(
                                                ty,
                                            )
                                        }),
                                });
                        }
                    }
                    AstItemKind::Extend(extend_block)
                        if dependency_type_expr_targets_struct(
                            &extend_block.target,
                            &symbol.name,
                        ) =>
                    {
                        for method in extend_block
                            .methods
                            .iter()
                            .filter(|method| is_public(&method.visibility))
                        {
                            let Some(definition_span) =
                                self.artifact_source_span(&module.source_path, method.name_span)
                            else {
                                continue;
                            };
                            extend_candidates
                                .entry(method.name.clone())
                                .or_default()
                                .push(DependencyStructResolvedMethod {
                                    name: method.name.clone(),
                                    source_path: module.source_path.clone(),
                                    detail: interface_detail_text(
                                        &module.contents,
                                        method.span,
                                        &method.name,
                                    ),
                                    return_type: method
                                        .return_type
                                        .as_ref()
                                        .map(render_dependency_type_expr),
                                    definition_span,
                                    return_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| self.public_type_target_for_type_expr(ty)),
                                    question_return_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| {
                                            self.public_question_inner_type_target_for_type_expr(ty)
                                        }),
                                    iterable_element_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| {
                                            self.public_iterable_element_type_target_for_type_expr(
                                                ty,
                                            )
                                        }),
                                    question_iterable_element_type_definition: method
                                        .return_type
                                        .as_ref()
                                        .and_then(|ty| {
                                            self.public_question_inner_iterable_element_type_target_for_type_expr(
                                                ty,
                                            )
                                        }),
                                });
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut methods = HashMap::new();
        for (name, candidates) in impl_candidates {
            if candidates.len() == 1 {
                methods.insert(name, candidates.into_iter().next().unwrap());
            }
        }
        for (name, candidates) in extend_candidates {
            if methods.contains_key(&name) || candidates.len() != 1 {
                continue;
            }
            methods.insert(name, candidates.into_iter().next().unwrap());
        }
        methods
    }
}

impl PackageAnalysis {
    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn modules(&self) -> &[PackageModuleAnalysis] {
        &self.modules
    }

    pub fn dependencies(&self) -> &[DependencyInterface] {
        &self.dependencies
    }

    pub fn public_enum_variant_completions(
        &self,
        source_path: &str,
        enum_name: &str,
    ) -> Option<Vec<CompletionItem>> {
        let module = package_module_for_source_path(self, source_path)?;
        let module_source = fs::read_to_string(module.path())
            .ok()?
            .replace("\r\n", "\n");
        enum_variant_completions_in_module_source(&module_source, enum_name)
    }

    pub fn public_enum_variant_completions_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        enum_name: &str,
    ) -> Option<Vec<CompletionItem>> {
        package_module_for_source_path(self, source_path)?;
        enum_variant_completions_in_module_source(module_source, enum_name)
    }

    pub fn public_struct_literal_field_completions_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
        excluded_field_names: &[String],
    ) -> Option<Vec<CompletionItem>> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        let excluded = excluded_field_names.iter().collect::<HashSet<_>>();
        let items = binding
            .fields
            .values()
            .filter(|field| !excluded.contains(&field.name))
            .map(|field| CompletionItem {
                label: field.name.clone(),
                insert_text: field.name.clone(),
                kind: SymbolKind::Field,
                detail: field.detail.clone(),
                ty: Some(field.ty.clone()),
            })
            .collect::<Vec<_>>();
        (!items.is_empty()).then_some(items)
    }

    pub fn public_struct_literal_field_completions(
        &self,
        source_path: &str,
        struct_name: &str,
        excluded_field_names: &[String],
    ) -> Option<Vec<CompletionItem>> {
        let binding = public_struct_completion_binding(self, source_path, struct_name)?;
        let excluded = excluded_field_names.iter().collect::<HashSet<_>>();
        let items = binding
            .fields
            .values()
            .filter(|field| !excluded.contains(&field.name))
            .map(|field| CompletionItem {
                label: field.name.clone(),
                insert_text: field.name.clone(),
                kind: SymbolKind::Field,
                detail: field.detail.clone(),
                ty: Some(field.ty.clone()),
            })
            .collect::<Vec<_>>();
        (!items.is_empty()).then_some(items)
    }

    pub fn public_struct_member_field_completions(
        &self,
        source_path: &str,
        struct_name: &str,
    ) -> Option<Vec<CompletionItem>> {
        let binding = public_struct_completion_binding(self, source_path, struct_name)?;
        let mut items = binding
            .fields
            .values()
            .map(|field| CompletionItem {
                label: field.name.clone(),
                insert_text: field.name.clone(),
                kind: SymbolKind::Field,
                detail: field.detail.clone(),
                ty: Some(field.ty.clone()),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub fn public_struct_member_field_completions_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
    ) -> Option<Vec<CompletionItem>> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        let mut items = binding
            .fields
            .values()
            .map(|field| CompletionItem {
                label: field.name.clone(),
                insert_text: field.name.clone(),
                kind: SymbolKind::Field,
                detail: field.detail.clone(),
                ty: Some(field.ty.clone()),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub fn public_struct_method_completions(
        &self,
        source_path: &str,
        struct_name: &str,
    ) -> Option<Vec<CompletionItem>> {
        let binding = public_struct_completion_binding(self, source_path, struct_name)?;
        let mut items = binding
            .methods
            .values()
            .map(|method| CompletionItem {
                label: method.name.clone(),
                insert_text: method.name.clone(),
                kind: SymbolKind::Method,
                detail: method.detail.clone(),
                ty: method.return_type.clone(),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub fn public_struct_method_completions_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
    ) -> Option<Vec<CompletionItem>> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        let mut items = binding
            .methods
            .values()
            .map(|method| CompletionItem {
                label: method.name.clone(),
                insert_text: method.name.clone(),
                kind: SymbolKind::Method,
                detail: method.detail.clone(),
                ty: method.return_type.clone(),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub fn public_struct_member_field_definition_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
        field_name: &str,
    ) -> Option<DependencyDefinitionTarget> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        let field = binding.fields.get(field_name)?.clone();
        Some(DependencyDefinitionTarget {
            package_name: binding.package_name.clone(),
            manifest_path: binding.manifest_path.clone(),
            source_path: binding.source_path.clone(),
            kind: SymbolKind::Field,
            name: field.name,
            path: binding.path.clone(),
            span: field.definition_span,
        })
    }

    pub fn public_struct_member_field_type_definition_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
        field_name: &str,
    ) -> Option<DependencyDefinitionTarget> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        binding.fields.get(field_name)?.type_definition.clone()
    }

    pub fn public_struct_method_definition_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
        method_name: &str,
    ) -> Option<DependencyDefinitionTarget> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        let method = binding.methods.get(method_name)?.clone();
        Some(DependencyDefinitionTarget {
            package_name: binding.package_name.clone(),
            manifest_path: binding.manifest_path.clone(),
            source_path: method.source_path,
            kind: SymbolKind::Method,
            name: method.name,
            path: binding.path.clone(),
            span: method.definition_span,
        })
    }

    pub fn public_struct_method_type_definition_in_source(
        &self,
        source_path: &str,
        module_source: &str,
        struct_name: &str,
        method_name: &str,
    ) -> Option<DependencyDefinitionTarget> {
        let binding = public_struct_completion_binding_in_source(
            self,
            source_path,
            module_source,
            struct_name,
        )?;
        binding
            .methods
            .get(method_name)?
            .return_type_definition
            .clone()
    }

    pub fn dependency_variant_completion_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyEnumCompletionTarget> {
        let module = parse_source(source).ok()?;
        let root_offset = dependency_variant_completion_root_offset(source, offset)?;
        let root_end = dependency_identifier_end(source, root_offset);
        let root_name = source.get(root_offset..root_end)?;
        let (dependency, symbol) =
            dependency_import_binding_for_local_name(self, &module, root_name)?;
        (symbol.kind == SymbolKind::Enum).then(|| DependencyEnumCompletionTarget {
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            enum_name: symbol.name.clone(),
            path: dependency.interface_path.clone(),
        })
    }

    pub fn dependency_struct_field_completion_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyStructFieldCompletionTarget> {
        let module = parse_source(source).ok()?;
        let site = dependency_struct_field_completion_site(&module, offset)?;
        let (dependency, symbol) =
            dependency_struct_import_binding_for_local_name(self, &module, &site.root_name)?;
        Some(DependencyStructFieldCompletionTarget {
            target: dependency_struct_completion_target(dependency, symbol)?,
            excluded_field_names: site.excluded_field_names,
        })
    }

    pub fn dependency_method_completion_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyStructCompletionTarget> {
        dependency_member_completion_target_in_source_at(
            self,
            source,
            offset,
            DependencyMemberCompletionKind::Method,
        )
        .or_else(|| {
            dependency_member_completion_target_in_broken_source(
                self,
                source,
                offset,
                DependencyMemberCompletionKind::MethodReceiver,
            )
        })
    }

    pub fn dependency_member_field_completion_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyStructCompletionTarget> {
        dependency_member_completion_target_in_source_at(
            self,
            source,
            offset,
            DependencyMemberCompletionKind::Field,
        )
        .or_else(|| {
            dependency_member_completion_target_in_broken_source(
                self,
                source,
                offset,
                DependencyMemberCompletionKind::FieldReceiver,
            )
        })
    }

    fn dependency_value_binding_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyStructBinding> {
        let module = parse_source(source).ok()?;
        dependency_member_completion_binding(
            self,
            &module,
            source,
            offset,
            DependencyMemberCompletionKind::ValueType,
        )
    }

    fn dependency_value_root_binding_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyStructBinding> {
        self.dependency_value_binding_in_source_at(source, offset)
            .or_else(|| {
                let module = parse_source(source).ok()?;
                dependency_value_root_binding_in_module(self, &module, offset)
            })
    }

    pub fn dependency_symbols(&self) -> Vec<&DependencySymbol> {
        self.dependencies
            .iter()
            .flat_map(|dependency| dependency.symbols())
            .collect()
    }

    pub fn dependency_symbols_named(&self, name: &str) -> Vec<&DependencySymbol> {
        self.dependencies
            .iter()
            .flat_map(|dependency| dependency.symbols_named(name))
            .collect()
    }

    pub fn dependency_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let context = dependency_import_completion_context(source, offset)?;
        let mut items = self
            .dependencies
            .iter()
            .flat_map(|dependency| dependency_completion_items(dependency, &context))
            .collect::<Vec<_>>();

        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        items.dedup_by(|left, right| {
            left.label == right.label && left.detail == right.detail && left.kind == right.kind
        });
        Some(items)
    }

    pub fn dependency_variant_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let root_offset = dependency_variant_completion_root_offset(source, offset)?;
        match parse_source(source) {
            Ok(module) => {
                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (dependency, symbol) =
                    dependency_import_binding_for_local_name(self, &module, root_name)?;
                dependency.variant_completions_for(symbol)
            }
            Err(_) => {
                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (tokens, _) = lex(source);
                let (_, target) = dependency_resolved_import_targets_in_tokens(self, &tokens)
                    .get(root_name)?
                    .clone();
                if target.kind != SymbolKind::Enum {
                    return None;
                }

                let (dependency, symbol) =
                    dependency_symbol_for_broken_source_target(self, &target)?;
                if symbol.kind != SymbolKind::Enum {
                    return None;
                }

                dependency.variant_completions_for(symbol)
            }
        }
    }

    pub fn dependency_struct_field_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match parse_source(source) {
            Ok(module) => {
                let site = dependency_struct_field_completion_site(&module, offset)?;
                let (dependency, symbol) = dependency_struct_import_binding_for_local_name(
                    self,
                    &module,
                    &site.root_name,
                )?;
                dependency_struct_field_completion_items(
                    dependency,
                    symbol,
                    &site.excluded_field_names,
                )
            }
            Err(_) => {
                let (tokens, _) = lex(source);
                let site = dependency_struct_field_completion_site_in_broken_source_tokens(
                    &tokens, offset,
                )?;
                let (_, target) = dependency_resolved_import_targets_in_tokens(self, &tokens)
                    .get(site.root_name.as_str())?
                    .clone();
                let (dependency, symbol) =
                    dependency_symbol_for_broken_source_target(self, &target)?;
                if symbol.kind != SymbolKind::Struct {
                    return None;
                }

                dependency_struct_field_completion_items(
                    dependency,
                    symbol,
                    &site.excluded_field_names,
                )
            }
        }
    }

    pub fn dependency_method_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let binding = match parse_source(source) {
            Ok(module) => dependency_member_completion_binding(
                self,
                &module,
                source,
                offset,
                DependencyMemberCompletionKind::Method,
            ),
            Err(_) => dependency_member_completion_binding_in_broken_source(
                self,
                source,
                offset,
                DependencyMemberCompletionKind::Method,
            ),
        }?;
        let mut items = binding
            .methods
            .values()
            .map(|method| CompletionItem {
                label: method.name.clone(),
                insert_text: method.name.clone(),
                kind: SymbolKind::Method,
                detail: method.detail.clone(),
                ty: method.return_type.clone(),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub fn dependency_member_field_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let binding = match parse_source(source) {
            Ok(module) => dependency_member_completion_binding(
                self,
                &module,
                source,
                offset,
                DependencyMemberCompletionKind::Field,
            ),
            Err(_) => dependency_member_completion_binding_in_broken_source(
                self,
                source,
                offset,
                DependencyMemberCompletionKind::Field,
            ),
        }?;
        let mut items = binding
            .fields
            .values()
            .map(|field| CompletionItem {
                label: field.name.clone(),
                insert_text: field.name.clone(),
                kind: SymbolKind::Field,
                detail: field.detail.clone(),
                ty: Some(field.ty.clone()),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.detail.cmp(&right.detail))
        });
        Some(items)
    }

    pub fn dependency_variant_hover_at(
        &self,
        analysis: &Analysis,
        source: &str,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_variant_target_at(analysis, source, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Variant,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_variant_hover_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_variant_target_in_source_at(source, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Variant,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_variant_definition_at(
        &self,
        analysis: &Analysis,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_variant_target_at(analysis, source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Variant,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_variant_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_variant_target_in_source_at(source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Variant,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_variant_references_at(
        &self,
        analysis: &Analysis,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let target = self.dependency_variant_target_at(analysis, source, offset)?;
        let mut references = source
            .match_indices(&target.name)
            .filter_map(|(start, _)| {
                let (root_offset, span, variant_name) =
                    dependency_variant_reference_at(source, start)?;
                if span.start != start || variant_name != target.name {
                    return None;
                }

                let (binding, _) = analysis.import_binding_at(root_offset)?;
                let (dependency, symbol) = self.resolve_dependency_import_binding(&binding)?;
                if dependency.interface_path != target.path
                    || dependency.artifact.package_name != target.package_name
                    || symbol.kind != SymbolKind::Enum
                    || symbol.source_path != target.source_path
                    || symbol.name != target.enum_name
                {
                    return None;
                }

                Some(ReferenceTarget {
                    kind: SymbolKind::Variant,
                    name: target.name.clone(),
                    span,
                    is_definition: false,
                })
            })
            .collect::<Vec<_>>();
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    pub fn dependency_variant_references_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => return self.dependency_variant_references_in_broken_source(source, offset),
        };
        let target = self.dependency_variant_target_in_source_at(source, offset)?;
        let mut references = source
            .match_indices(&target.name)
            .filter_map(|(start, _)| {
                let (root_offset, span, variant_name) =
                    dependency_variant_reference_at(source, start)?;
                if span.start != start || variant_name != target.name {
                    return None;
                }

                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (dependency, symbol) =
                    dependency_import_binding_for_local_name(self, &module, root_name)?;
                if dependency.interface_path != target.path
                    || dependency.artifact.package_name != target.package_name
                    || symbol.kind != SymbolKind::Enum
                    || symbol.source_path != target.source_path
                    || symbol.name != target.enum_name
                {
                    return None;
                }

                Some(ReferenceTarget {
                    kind: SymbolKind::Variant,
                    name: target.name.clone(),
                    span,
                    is_definition: false,
                })
            })
            .collect::<Vec<_>>();
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    fn dependency_variant_references_in_broken_source(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let target = self.dependency_variant_target_in_broken_source(source, offset)?;
        let (tokens, _) = lex(source);
        let import_targets = dependency_resolved_import_targets_in_tokens(self, &tokens);
        let mut references = source
            .match_indices(&target.name)
            .filter_map(|(start, _)| {
                let (root_offset, span, variant_name) =
                    dependency_variant_reference_at(source, start)?;
                if span.start != start || variant_name != target.name {
                    return None;
                }

                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (_, resolved_target) = import_targets.get(root_name)?;
                if resolved_target.kind != SymbolKind::Enum {
                    return None;
                }

                let (dependency, symbol) =
                    dependency_symbol_for_broken_source_target(self, resolved_target)?;
                if dependency.interface_path != target.path
                    || dependency.manifest.manifest_path != target.manifest_path
                    || symbol.kind != SymbolKind::Enum
                    || symbol.source_path != target.source_path
                    || symbol.name != target.enum_name
                {
                    return None;
                }

                dependency.variant_for(symbol, &variant_name)?;

                Some(ReferenceTarget {
                    kind: SymbolKind::Variant,
                    name: target.name.clone(),
                    span,
                    is_definition: false,
                })
            })
            .collect::<Vec<_>>();
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    pub fn dependency_struct_field_hover_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_struct_field_target_at(analysis, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Field,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_method_hover_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_method_target_at(analysis, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Method,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_method_hover_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_method_target_in_source_at(source, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Method,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_struct_field_hover_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Field,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_struct_field_definition_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_struct_field_target_at(analysis, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Field,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_method_definition_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_method_target_at(analysis, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Method,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_method_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_method_target_in_source_at(source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Method,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_struct_field_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Field,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_struct_field_references_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let target = self.dependency_struct_field_target_at(analysis, offset)?;
        let mut references = self
            .dependency_struct_field_occurrences(analysis.ast())
            .into_iter()
            .filter(|occurrence| {
                occurrence.package_name == target.package_name
                    && occurrence.source_path == target.source_path
                    && occurrence.struct_name == target.struct_name
                    && occurrence.name == target.name
                    && occurrence.path == target.path
            })
            .map(|occurrence| ReferenceTarget {
                kind: SymbolKind::Field,
                name: occurrence.name,
                span: occurrence.reference_span,
                is_definition: false,
            })
            .collect::<Vec<_>>();
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    pub fn dependency_method_references_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let target = self.dependency_method_target_at(analysis, offset)?;
        let mut references = self
            .dependency_method_occurrences(analysis.ast())
            .into_iter()
            .filter(|occurrence| {
                occurrence.package_name == target.package_name
                    && occurrence.source_path == target.source_path
                    && occurrence.struct_name == target.struct_name
                    && occurrence.name == target.name
                    && occurrence.path == target.path
            })
            .map(|occurrence| ReferenceTarget {
                kind: SymbolKind::Method,
                name: occurrence.name,
                span: occurrence.reference_span,
                is_definition: false,
            })
            .collect::<Vec<_>>();
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    pub fn dependency_method_references_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let mut references = if let Ok(module) = parse_source(source) {
            let target = self.dependency_method_target_in_source_at(source, offset)?;
            self.dependency_method_occurrences(&module)
                .into_iter()
                .filter(|occurrence| {
                    occurrence.package_name == target.package_name
                        && occurrence.source_path == target.source_path
                        && occurrence.struct_name == target.struct_name
                        && occurrence.name == target.name
                        && occurrence.path == target.path
                })
                .map(|occurrence| ReferenceTarget {
                    kind: SymbolKind::Method,
                    name: occurrence.name,
                    span: occurrence.reference_span,
                    is_definition: false,
                })
                .collect::<Vec<_>>()
        } else {
            Self::dependency_method_references_in_broken_source(self, source, offset)?
        };
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    pub fn dependency_struct_field_references_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let mut references = if let Ok(module) = parse_source(source) {
            let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
            self.dependency_struct_field_occurrences(&module)
                .into_iter()
                .filter(|occurrence| {
                    occurrence.package_name == target.package_name
                        && occurrence.source_path == target.source_path
                        && occurrence.struct_name == target.struct_name
                        && occurrence.name == target.name
                        && occurrence.path == target.path
                })
                .map(|occurrence| ReferenceTarget {
                    kind: SymbolKind::Field,
                    name: occurrence.name,
                    span: occurrence.reference_span,
                    is_definition: false,
                })
                .collect::<Vec<_>>()
        } else {
            Self::dependency_struct_field_references_in_broken_source(self, source, offset)?
        };
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    /// Return dependency-backed semantic-token occurrences for the current source.
    ///
    /// This augments same-file semantic tokens with dependency-aware value/member/variant surfaces
    /// that require package interface knowledge.
    pub fn dependency_semantic_tokens_in_source(
        &self,
        source: &str,
    ) -> Vec<SemanticTokenOccurrence> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => return Vec::new(),
        };

        let mut tokens = collect_dependency_semantic_tokens_in_module(self, &module, source);
        sort_and_dedup_semantic_tokens(&mut tokens);
        tokens
    }

    pub fn dependency_import_root_semantic_tokens_in_source(
        &self,
        source: &str,
    ) -> Vec<SemanticTokenOccurrence> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => return Vec::new(),
        };

        let mut tokens =
            collect_dependency_import_root_semantic_tokens_in_module(self, &module, source);
        sort_and_dedup_semantic_tokens(&mut tokens);
        tokens
    }

    /// Return dependency-backed semantic-token occurrences that remain available even when
    /// same-file semantic analysis failed or the current source contains parse errors.
    pub fn dependency_fallback_semantic_tokens_in_source(
        &self,
        source: &str,
    ) -> Vec<SemanticTokenOccurrence> {
        let mut tokens = match parse_source(source) {
            Ok(module) => {
                let mut tokens =
                    collect_dependency_semantic_tokens_in_module(self, &module, source);
                tokens.extend(collect_dependency_import_root_semantic_tokens_in_module(
                    self, &module, source,
                ));
                tokens
            }
            Err(_) => {
                let mut tokens = collect_dependency_semantic_tokens_in_broken_source(self, source);
                tokens.extend(
                    collect_dependency_import_root_semantic_tokens_in_broken_source(self, source),
                );
                tokens
            }
        };
        sort_and_dedup_semantic_tokens(&mut tokens);
        tokens
    }

    pub fn dependency_hover_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_target_at(analysis, offset)?;
        Some(DependencyHoverInfo {
            span: target.import_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: target.kind,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_definition_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_target_at(analysis, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: target.kind,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_type_definition_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let binding = analysis.type_import_binding_at(offset)?;
        let (dependency, symbol) = self.resolve_dependency_import_binding(&binding)?;
        let definition_span = dependency.artifact_span_for(symbol)?;
        Some(DependencyDefinitionTarget {
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            path: dependency.interface_path.clone(),
            span: definition_span,
        })
    }

    pub fn dependency_hover_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_target_in_source_at(source, offset)?;
        Some(DependencyHoverInfo {
            span: target.import_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: target.kind,
            name: target.name,
            detail: target.detail,
        })
    }

    pub fn dependency_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_target_in_source_at(source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: target.kind,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_type_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_type_target_in_source_at(source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: target.kind,
            name: target.name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_value_type_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_value_target_in_source_at(source, offset)?;
        Some(DependencyDefinitionTarget {
            package_name: target.package_name,
            manifest_path: target.manifest_path,
            source_path: target.source_path,
            kind: SymbolKind::Struct,
            name: target.struct_name,
            path: target.path,
            span: target.definition_span,
        })
    }

    pub fn dependency_value_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        self.dependency_value_type_definition_in_source_at(source, offset)
    }

    pub fn dependency_value_hover_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let target = self.dependency_value_target_in_source_at(source, offset)?;
        Some(DependencyHoverInfo {
            span: target.reference_span,
            package_name: target.package_name,
            source_path: target.source_path,
            kind: SymbolKind::Struct,
            name: target.struct_name,
            detail: target.detail,
        })
    }

    pub fn dependency_variant_type_definition_at(
        &self,
        analysis: &Analysis,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let (root_offset, _, variant_name) = dependency_variant_reference_at(source, offset)?;
        let (binding, _) = analysis.import_binding_at(root_offset)?;
        let (dependency, symbol) = self.resolve_dependency_import_binding(&binding)?;
        dependency.variant_for(symbol, &variant_name)?;
        let definition_span = dependency.artifact_span_for(symbol)?;
        Some(DependencyDefinitionTarget {
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            kind: SymbolKind::Enum,
            name: symbol.name.clone(),
            path: dependency.interface_path.clone(),
            span: definition_span,
        })
    }

    pub fn dependency_variant_type_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let (root_offset, _, variant_name) = dependency_variant_reference_at(source, offset)?;
        match parse_source(source) {
            Ok(module) => {
                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (dependency, symbol) =
                    dependency_import_binding_for_local_name(self, &module, root_name)?;
                dependency.variant_for(symbol, &variant_name)?;
                let definition_span = dependency.artifact_span_for(symbol)?;
                Some(DependencyDefinitionTarget {
                    package_name: dependency.artifact.package_name.clone(),
                    manifest_path: dependency.manifest.manifest_path.clone(),
                    source_path: symbol.source_path.clone(),
                    kind: SymbolKind::Enum,
                    name: symbol.name.clone(),
                    path: dependency.interface_path.clone(),
                    span: definition_span,
                })
            }
            Err(_) => {
                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (tokens, _) = lex(source);
                let (_, target) = dependency_resolved_import_targets_in_tokens(self, &tokens)
                    .get(root_name)?
                    .clone();
                if target.kind != SymbolKind::Enum {
                    return None;
                }

                let (dependency, symbol) =
                    dependency_symbol_for_broken_source_target(self, &target)?;
                if symbol.kind != SymbolKind::Enum {
                    return None;
                }

                dependency.variant_for(symbol, &variant_name)?;
                let definition_span = dependency.artifact_span_for(symbol)?;
                Some(DependencyDefinitionTarget {
                    package_name: dependency.artifact.package_name.clone(),
                    manifest_path: dependency.manifest.manifest_path.clone(),
                    source_path: symbol.source_path.clone(),
                    kind: SymbolKind::Enum,
                    name: symbol.name.clone(),
                    path: dependency.interface_path.clone(),
                    span: definition_span,
                })
            }
        }
    }

    pub fn dependency_struct_field_type_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
        let dependency = self
            .dependencies
            .iter()
            .find(|dependency| dependency.interface_path == target.path)?;
        let symbol = dependency.symbols.iter().find(|symbol| {
            symbol.kind == SymbolKind::Struct
                && symbol.source_path == target.source_path
                && symbol.name == target.struct_name
        })?;
        let field = dependency.struct_field_for(symbol, &target.name)?;
        let question_wrapped = match parse_source(source) {
            Ok(module) => dependency_question_wrapped_field_reference_in_module(&module, offset),
            Err(_) => dependency_question_wrapped_field_reference_in_broken_source(source, offset),
        };
        if question_wrapped {
            dependency
                .public_question_inner_type_target_for_type_expr(&field.ty)
                .or_else(|| dependency.public_type_target_for_type_expr(&field.ty))
        } else {
            dependency.public_type_target_for_type_expr(&field.ty)
        }
    }

    pub fn dependency_method_type_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_method_target_in_source_at(source, offset)?;
        let dependency = self
            .dependencies
            .iter()
            .find(|dependency| dependency.interface_path == target.path)?;
        let method = dependency
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == SymbolKind::Struct && symbol.name == target.struct_name)
            .find_map(|symbol| {
                let mut methods = dependency.struct_methods_for(symbol);
                let method = methods.get(&target.name)?;
                (method.source_path == target.source_path
                    && method.definition_span == target.definition_span)
                    .then(|| methods.remove(&target.name))
                    .flatten()
            })?;
        let question_wrapped = match parse_source(source) {
            Ok(module) => dependency_question_wrapped_method_reference_in_module(&module, offset),
            Err(_) => dependency_question_wrapped_method_reference_in_broken_source(source, offset),
        };
        if question_wrapped {
            method
                .question_return_type_definition
                .or(method.return_type_definition)
        } else {
            method.return_type_definition
        }
    }

    pub fn dependency_references_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let mut references = if let Ok(module) = parse_source(source) {
            let target_occurrence = dependency_import_occurrence_in_module(&module, offset)?;
            let (dependency, symbol) = dependency_import_binding_for_local_name(
                self,
                &module,
                &target_occurrence.local_name,
            )?;
            source
                .match_indices(&target_occurrence.local_name)
                .filter_map(|(start, _)| {
                    let occurrence = dependency_import_occurrence_in_module(&module, start)?;
                    if occurrence.span.start != start
                        || occurrence.local_name != target_occurrence.local_name
                    {
                        return None;
                    }

                    let (occurrence_dependency, occurrence_symbol) =
                        dependency_import_binding_for_local_name(
                            self,
                            &module,
                            &occurrence.local_name,
                        )?;
                    if occurrence_dependency.interface_path != dependency.interface_path
                        || occurrence_dependency.artifact.package_name
                            != dependency.artifact.package_name
                        || occurrence_symbol.source_path != symbol.source_path
                        || occurrence_symbol.kind != symbol.kind
                        || occurrence_symbol.name != symbol.name
                    {
                        return None;
                    }

                    Some(ReferenceTarget {
                        kind: symbol.kind,
                        name: symbol.name.clone(),
                        span: occurrence.span,
                        is_definition: occurrence.is_definition,
                    })
                })
                .collect::<Vec<_>>()
        } else {
            let occurrences = dependency_import_occurrences_in_broken_source(self, source);
            let target_occurrence = occurrences
                .iter()
                .find(|occurrence| occurrence.span.contains(offset))?
                .clone();
            occurrences
                .into_iter()
                .filter(|occurrence| {
                    occurrence.local_name == target_occurrence.local_name
                        && occurrence.target.path == target_occurrence.target.path
                        && occurrence.target.package_name == target_occurrence.target.package_name
                        && occurrence.target.source_path == target_occurrence.target.source_path
                        && occurrence.target.kind == target_occurrence.target.kind
                        && occurrence.target.name == target_occurrence.target.name
                })
                .map(|occurrence| ReferenceTarget {
                    kind: occurrence.target.kind,
                    name: occurrence.target.name,
                    span: occurrence.span,
                    is_definition: occurrence.is_definition,
                })
                .collect::<Vec<_>>()
        };
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    pub fn dependency_prepare_rename_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<RenameTarget> {
        let Ok(module) = parse_source(source) else {
            return dependency_import_occurrence_in_broken_source(self, source, offset)
                .map(|occurrence| RenameTarget {
                    kind: SymbolKind::Import,
                    name: occurrence.local_name,
                    span: occurrence.span,
                })
                .or_else(|| {
                    let occurrence =
                        dependency_value_occurrence_in_broken_source(self, source, offset)?;
                    dependency_value_occurrence_supports_same_file_rename(&occurrence).then_some(
                        RenameTarget {
                            kind: occurrence.kind,
                            name: occurrence.local_name,
                            span: occurrence.reference_span,
                        },
                    )
                })
                .or_else(|| {
                    self.dependency_variant_target_in_source_at(source, offset)
                        .map(|target| RenameTarget {
                            kind: SymbolKind::Variant,
                            name: target.name,
                            span: target.reference_span,
                        })
                })
                .or_else(|| {
                    self.dependency_method_target_in_source_at(source, offset)
                        .map(|target| RenameTarget {
                            kind: SymbolKind::Method,
                            name: target.name,
                            span: target.reference_span,
                        })
                })
                .or_else(|| {
                    self.dependency_struct_field_target_in_source_at(source, offset)
                        .map(|target| RenameTarget {
                            kind: SymbolKind::Field,
                            name: target.name,
                            span: target.reference_span,
                        })
                });
        };
        if let Some(occurrence) = dependency_import_occurrence_in_module(&module, offset) {
            if let Some(binding) =
                dependency_unique_import_binding_for_local_name(&module, &occurrence.local_name)
                && self.resolve_dependency_import_binding(&binding).is_some()
            {
                return Some(RenameTarget {
                    kind: SymbolKind::Import,
                    name: occurrence.local_name,
                    span: occurrence.span,
                });
            }
        }

        if let Some(occurrence) = self.dependency_value_occurrence_in_module(&module, offset)
            && dependency_value_occurrence_supports_same_file_rename(&occurrence)
        {
            return Some(RenameTarget {
                kind: occurrence.kind,
                name: occurrence.local_name,
                span: occurrence.reference_span,
            });
        }

        self.dependency_variant_target_in_source_at(source, offset)
            .map(|target| RenameTarget {
                kind: SymbolKind::Variant,
                name: target.name,
                span: target.reference_span,
            })
            .or_else(|| {
                self.dependency_method_target_in_source_at(source, offset)
                    .map(|target| RenameTarget {
                        kind: SymbolKind::Method,
                        name: target.name,
                        span: target.reference_span,
                    })
            })
            .or_else(|| {
                self.dependency_struct_field_target_in_source_at(source, offset)
                    .map(|target| RenameTarget {
                        kind: SymbolKind::Field,
                        name: target.name,
                        span: target.reference_span,
                    })
            })
    }

    pub fn dependency_rename_in_source_at(
        &self,
        source: &str,
        offset: usize,
        new_name: &str,
    ) -> Result<Option<RenameResult>, RenameError> {
        validate_dependency_rename_text(new_name)?;

        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => {
                return Ok(self
                    .dependency_import_rename_in_broken_source(source, offset, new_name)
                    .or_else(|| {
                        self.dependency_value_rename_in_broken_source(source, offset, new_name)
                    })
                    .or_else(|| self.dependency_variant_rename_in_source(source, offset, new_name))
                    .or_else(|| self.dependency_method_rename_in_source(source, offset, new_name))
                    .or_else(|| {
                        self.dependency_struct_field_rename_in_source(source, offset, new_name)
                    }));
            }
        };
        if let Some(target_occurrence) = dependency_import_occurrence_in_module(&module, offset) {
            if let Some(target_binding) = dependency_unique_import_binding_for_local_name(
                &module,
                &target_occurrence.local_name,
            ) && let Some((dependency, symbol)) =
                self.resolve_dependency_import_binding(&target_binding)
            {
                let mut edits = source
                    .match_indices(&target_occurrence.local_name)
                    .filter_map(|(start, _)| {
                        let occurrence = dependency_import_occurrence_in_module(&module, start)?;
                        if occurrence.span.start != start
                            || occurrence.local_name != target_occurrence.local_name
                        {
                            return None;
                        }

                        let occurrence_binding = dependency_unique_import_binding_for_local_name(
                            &module,
                            &occurrence.local_name,
                        )?;
                        let (occurrence_dependency, occurrence_symbol) =
                            self.resolve_dependency_import_binding(&occurrence_binding)?;
                        if occurrence_dependency.interface_path != dependency.interface_path
                            || occurrence_dependency.artifact.package_name
                                != dependency.artifact.package_name
                            || occurrence_symbol.source_path != symbol.source_path
                            || occurrence_symbol.kind != symbol.kind
                            || occurrence_symbol.name != symbol.name
                        {
                            return None;
                        }

                        let replacement = if occurrence.is_definition
                            && dependency_import_binding_uses_direct_local_name(&occurrence_binding)
                        {
                            format!(
                                "{} as {}",
                                dependency_import_binding_imported_name(&occurrence_binding)?,
                                new_name
                            )
                        } else {
                            new_name.to_owned()
                        };
                        Some(RenameEdit {
                            span: occurrence.span,
                            replacement,
                        })
                    })
                    .collect::<Vec<_>>();
                if edits.is_empty() {
                    return Ok(None);
                }
                edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

                return Ok(Some(RenameResult {
                    kind: SymbolKind::Import,
                    old_name: target_occurrence.local_name,
                    new_name: new_name.to_owned(),
                    edits,
                }));
            }
        }

        let Some(target_occurrence) = self.dependency_value_occurrence_in_module(&module, offset)
        else {
            return Ok(self
                .dependency_variant_rename_in_source(source, offset, new_name)
                .or_else(|| self.dependency_method_rename_in_source(source, offset, new_name))
                .or_else(|| {
                    self.dependency_struct_field_rename_in_source(source, offset, new_name)
                }));
        };
        if !dependency_value_occurrence_supports_same_file_rename(&target_occurrence) {
            return Ok(None);
        }

        let replacement = new_name.to_owned();
        let mut edits = self
            .dependency_value_occurrences(&module)
            .into_iter()
            .filter(|occurrence| {
                dependency_value_occurrence_matches_rename_target(occurrence, &target_occurrence)
            })
            .map(|occurrence| RenameEdit {
                span: occurrence.reference_span,
                replacement: dependency_value_occurrence_rename_replacement(
                    &occurrence,
                    replacement.as_str(),
                ),
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return Ok(None);
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

        Ok(Some(RenameResult {
            kind: target_occurrence.kind,
            old_name: target_occurrence.local_name,
            new_name: new_name.to_owned(),
            edits,
        }))
    }

    fn dependency_variant_rename_in_source(
        &self,
        source: &str,
        offset: usize,
        new_name: &str,
    ) -> Option<RenameResult> {
        let target = self.dependency_variant_target_in_source_at(source, offset)?;
        let replacement = new_name.to_owned();
        let mut edits = self
            .dependency_variant_references_in_source_at(source, offset)?
            .into_iter()
            .map(|reference| RenameEdit {
                span: reference.span,
                replacement: replacement.clone(),
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return None;
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

        Some(RenameResult {
            kind: SymbolKind::Variant,
            old_name: target.name,
            new_name: new_name.to_owned(),
            edits,
        })
    }

    fn dependency_import_rename_in_broken_source(
        &self,
        source: &str,
        offset: usize,
        new_name: &str,
    ) -> Option<RenameResult> {
        let target_occurrence =
            dependency_import_occurrence_in_broken_source(self, source, offset)?;
        let target_local_name = target_occurrence.local_name.clone();
        let target = target_occurrence.target.clone();
        let (tokens, _) = lex(source);
        let resolved_imports = dependency_resolved_import_targets_in_tokens(self, &tokens);
        let (binding, _) = resolved_imports.get(target_local_name.as_str())?;
        let imported_name = dependency_import_binding_uses_direct_local_name(binding)
            .then(|| dependency_import_binding_imported_name(binding))
            .flatten();
        let mut edits = dependency_import_occurrences_in_broken_source(self, source)
            .into_iter()
            .filter(|occurrence| {
                occurrence.local_name == target_local_name && occurrence.target == target
            })
            .map(|occurrence| RenameEdit {
                span: occurrence.span,
                replacement: if occurrence.is_definition {
                    imported_name
                        .map(|imported_name| format!("{imported_name} as {new_name}"))
                        .unwrap_or_else(|| new_name.to_owned())
                } else {
                    new_name.to_owned()
                },
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return None;
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

        Some(RenameResult {
            kind: SymbolKind::Import,
            old_name: target_local_name,
            new_name: new_name.to_owned(),
            edits,
        })
    }

    fn dependency_value_rename_in_broken_source(
        &self,
        source: &str,
        offset: usize,
        new_name: &str,
    ) -> Option<RenameResult> {
        let target_occurrence = dependency_value_occurrence_in_broken_source(self, source, offset)?;
        if !dependency_value_occurrence_supports_same_file_rename(&target_occurrence) {
            return None;
        }

        let replacement = new_name.to_owned();
        let mut edits = dependency_value_occurrences_in_broken_source(self, source)
            .into_iter()
            .filter(|occurrence| {
                dependency_value_occurrence_matches_rename_target(occurrence, &target_occurrence)
            })
            .map(|occurrence| RenameEdit {
                span: occurrence.reference_span,
                replacement: dependency_value_occurrence_rename_replacement(
                    &occurrence,
                    replacement.as_str(),
                ),
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return None;
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

        Some(RenameResult {
            kind: target_occurrence.kind,
            old_name: target_occurrence.local_name,
            new_name: new_name.to_owned(),
            edits,
        })
    }

    fn dependency_method_rename_in_source(
        &self,
        source: &str,
        offset: usize,
        new_name: &str,
    ) -> Option<RenameResult> {
        let target = self.dependency_method_target_in_source_at(source, offset)?;
        let replacement = new_name.to_owned();
        let mut edits = self
            .dependency_method_references_in_source_at(source, offset)?
            .into_iter()
            .map(|reference| RenameEdit {
                span: reference.span,
                replacement: replacement.clone(),
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return None;
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

        Some(RenameResult {
            kind: SymbolKind::Method,
            old_name: target.name,
            new_name: new_name.to_owned(),
            edits,
        })
    }

    fn dependency_struct_field_rename_in_source(
        &self,
        source: &str,
        offset: usize,
        new_name: &str,
    ) -> Option<RenameResult> {
        let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
        let replacement = new_name.to_owned();
        let mut edits = self
            .dependency_struct_field_references_in_source_at(source, offset)?
            .into_iter()
            .map(|reference| RenameEdit {
                span: reference.span,
                replacement: replacement.clone(),
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            return None;
        }
        edits.sort_by_key(|edit| (edit.span.start, edit.span.end));

        Some(RenameResult {
            kind: SymbolKind::Field,
            old_name: target.name,
            new_name: new_name.to_owned(),
            edits,
        })
    }

    pub fn dependency_value_references_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let mut references = if let Ok(module) = parse_source(source) {
            if let Some(target_occurrence) =
                self.dependency_value_occurrence_in_module(&module, offset)
            {
                self.dependency_value_occurrences(&module)
                    .into_iter()
                    .filter(|occurrence| {
                        occurrence.local_name == target_occurrence.local_name
                            && occurrence.definition_span == target_occurrence.definition_span
                            && occurrence.package_name == target_occurrence.package_name
                            && occurrence.source_path == target_occurrence.source_path
                            && occurrence.struct_name == target_occurrence.struct_name
                            && occurrence.path == target_occurrence.path
                    })
                    .map(|occurrence| ReferenceTarget {
                        kind: occurrence.kind,
                        name: occurrence.local_name,
                        span: occurrence.reference_span,
                        is_definition: occurrence.is_definition,
                    })
                    .collect::<Vec<_>>()
            } else if let Some(target_occurrence) =
                dependency_import_occurrence_in_module(&module, offset)
            {
                let target_binding =
                    dependency_value_root_binding_in_module(self, &module, offset)?;
                source
                    .match_indices(&target_occurrence.local_name)
                    .filter_map(|(start, _)| {
                        let occurrence = dependency_import_occurrence_in_module(&module, start)?;
                        if occurrence.span.start != start
                            || occurrence.local_name != target_occurrence.local_name
                        {
                            return None;
                        }

                        if occurrence.is_definition {
                            if !dependency_value_root_import_binding_matches(
                                self,
                                &module,
                                &occurrence.local_name,
                                &target_binding,
                            ) {
                                return None;
                            }
                        } else {
                            let occurrence_binding =
                                dependency_value_root_binding_in_module(self, &module, start)?;
                            if !dependency_struct_bindings_match(
                                &occurrence_binding,
                                &target_binding,
                            ) {
                                return None;
                            }
                        }

                        Some(ReferenceTarget {
                            kind: SymbolKind::Struct,
                            name: target_occurrence.local_name.clone(),
                            span: occurrence.span,
                            is_definition: occurrence.is_definition,
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Self::dependency_value_references_in_broken_source(self, source, offset)?
            }
        } else {
            Self::dependency_value_references_in_broken_source(self, source, offset)?
        };
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
    }

    fn dependency_value_references_in_broken_source(
        package: &PackageAnalysis,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let occurrences = dependency_value_occurrences_in_broken_source(package, source);
        let target_occurrence = occurrences
            .iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))?
            .clone();
        Some(
            occurrences
                .into_iter()
                .filter(|occurrence| {
                    dependency_value_occurrence_matches_rename_target(
                        occurrence,
                        &target_occurrence,
                    )
                })
                .map(|occurrence| ReferenceTarget {
                    kind: occurrence.kind,
                    name: occurrence.local_name,
                    span: occurrence.reference_span,
                    is_definition: occurrence.is_definition,
                })
                .collect(),
        )
    }

    fn dependency_method_references_in_broken_source(
        package: &PackageAnalysis,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let target = dependency_method_target_in_broken_source(package, source, offset)?;
        let references = dependency_member_sites_in_broken_source(source)
            .into_iter()
            .filter(|site| site.member_kind == BrokenSourceValueSegmentKind::Method)
            .filter_map(|site| {
                let binding =
                    dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
                let method = binding.methods.get(&site.member_name)?;
                (binding.package_name == target.package_name
                    && method.source_path == target.source_path
                    && binding.struct_name == target.struct_name
                    && method.name == target.name
                    && binding.path == target.path
                    && method.definition_span == target.definition_span)
                    .then(|| ReferenceTarget {
                        kind: SymbolKind::Method,
                        name: method.name.clone(),
                        span: site.member_span,
                        is_definition: false,
                    })
            })
            .collect::<Vec<_>>();
        (!references.is_empty()).then_some(references)
    }

    fn dependency_struct_field_references_in_broken_source(
        package: &PackageAnalysis,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let target = dependency_struct_field_target_in_broken_source(package, source, offset)?;
        let references = dependency_member_sites_in_broken_source(source)
            .into_iter()
            .filter(|site| site.member_kind == BrokenSourceValueSegmentKind::Field)
            .filter_map(|site| {
                let binding =
                    dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
                let field = binding.fields.get(&site.member_name)?;
                (binding.package_name == target.package_name
                    && binding.source_path == target.source_path
                    && binding.struct_name == target.struct_name
                    && field.name == target.name
                    && binding.path == target.path
                    && field.definition_span == target.definition_span)
                    .then(|| ReferenceTarget {
                        kind: SymbolKind::Field,
                        name: field.name.clone(),
                        span: site.member_span,
                        is_definition: false,
                    })
            })
            .collect::<Vec<_>>();
        (!references.is_empty()).then_some(references)
    }

    pub fn dependency_target_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyResolvedTarget> {
        let (binding, import_span) = analysis.import_binding_at(offset)?;
        let (dependency, symbol) = self.resolve_dependency_import_binding(&binding)?;
        let definition_span = dependency.artifact_span_for(symbol)?;
        Some(DependencyResolvedTarget {
            import_span,
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            detail: symbol.detail.clone(),
            path: dependency.interface_path.clone(),
            definition_span,
        })
    }

    fn dependency_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyResolvedTarget> {
        if let Ok(module) = parse_source(source) {
            let occurrence = dependency_import_occurrence_in_module(&module, offset)?;
            let (dependency, symbol) =
                dependency_import_binding_for_local_name(self, &module, &occurrence.local_name)?;
            let definition_span = dependency.artifact_span_for(symbol)?;
            return Some(DependencyResolvedTarget {
                import_span: occurrence.span,
                package_name: dependency.artifact.package_name.clone(),
                manifest_path: dependency.manifest.manifest_path.clone(),
                source_path: symbol.source_path.clone(),
                kind: symbol.kind,
                name: symbol.name.clone(),
                detail: symbol.detail.clone(),
                path: dependency.interface_path.clone(),
                definition_span,
            });
        }

        dependency_import_occurrence_in_broken_source(self, source, offset)
            .map(|occurrence| occurrence.target)
    }

    fn dependency_type_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyResolvedTarget> {
        if let Ok(module) = parse_source(source) {
            let occurrence = dependency_type_import_occurrence_in_module(&module, offset)?;
            let (dependency, symbol) =
                dependency_import_binding_for_local_name(self, &module, &occurrence.local_name)?;
            if !matches!(
                symbol.kind,
                SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait | SymbolKind::TypeAlias
            ) {
                return None;
            }
            let definition_span = dependency.artifact_span_for(symbol)?;
            return Some(DependencyResolvedTarget {
                import_span: occurrence.span,
                package_name: dependency.artifact.package_name.clone(),
                manifest_path: dependency.manifest.manifest_path.clone(),
                source_path: symbol.source_path.clone(),
                kind: symbol.kind,
                name: symbol.name.clone(),
                detail: symbol.detail.clone(),
                path: dependency.interface_path.clone(),
                definition_span,
            });
        }

        dependency_import_occurrence_in_broken_source(self, source, offset).and_then(|occurrence| {
            matches!(
                occurrence.target.kind,
                SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait | SymbolKind::TypeAlias
            )
            .then_some(occurrence.target)
        })
    }

    fn resolve_dependency_import_binding<'a>(
        &'a self,
        binding: &ImportBinding,
    ) -> Option<(&'a DependencyInterface, &'a DependencySymbol)> {
        let imported_name = binding.path.segments.last()?;
        let mut matches = self
            .dependencies
            .iter()
            .filter(|dependency| dependency_matches_import(dependency, binding))
            .flat_map(|dependency| {
                dependency
                    .symbols()
                    .iter()
                    .filter(move |symbol| &symbol.name == imported_name)
                    .map(move |symbol| (dependency, symbol))
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return None;
        }
        matches.pop()
    }

    fn dependency_variant_target_at(
        &self,
        analysis: &Analysis,
        source: &str,
        offset: usize,
    ) -> Option<DependencyVariantTarget> {
        let (root_offset, reference_span, variant_name) =
            dependency_variant_reference_at(source, offset)?;
        let (binding, _) = analysis.import_binding_at(root_offset)?;
        let (dependency, symbol) = self.resolve_dependency_import_binding(&binding)?;
        let variant = dependency.variant_for(symbol, &variant_name)?;
        let definition_span =
            dependency.artifact_source_span(&symbol.source_path, variant.name_span)?;
        Some(DependencyVariantTarget {
            reference_span,
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            enum_name: symbol.name.clone(),
            name: variant.name.clone(),
            detail: dependency_variant_detail(&symbol.name, variant),
            path: dependency.interface_path.clone(),
            definition_span,
        })
    }

    fn dependency_variant_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyVariantTarget> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => return self.dependency_variant_target_in_broken_source(source, offset),
        };
        let (root_offset, reference_span, variant_name) =
            dependency_variant_reference_at(source, offset)?;
        let root_end = dependency_identifier_end(source, root_offset);
        let root_name = source.get(root_offset..root_end)?;
        let (dependency, symbol) =
            dependency_import_binding_for_local_name(self, &module, root_name)?;
        let variant = dependency.variant_for(symbol, &variant_name)?;
        let definition_span =
            dependency.artifact_source_span(&symbol.source_path, variant.name_span)?;
        Some(DependencyVariantTarget {
            reference_span,
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            enum_name: symbol.name.clone(),
            name: variant.name.clone(),
            detail: dependency_variant_detail(&symbol.name, variant),
            path: dependency.interface_path.clone(),
            definition_span,
        })
    }

    fn dependency_variant_target_in_broken_source(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyVariantTarget> {
        let (root_offset, reference_span, variant_name) =
            dependency_variant_reference_at(source, offset)?;
        let root_end = dependency_identifier_end(source, root_offset);
        let root_name = source.get(root_offset..root_end)?;
        let (tokens, _) = lex(source);
        let (_, target) = dependency_resolved_import_targets_in_tokens(self, &tokens)
            .get(root_name)?
            .clone();
        if target.kind != SymbolKind::Enum {
            return None;
        }

        let (dependency, symbol) = dependency_symbol_for_broken_source_target(self, &target)?;
        if symbol.kind != SymbolKind::Enum {
            return None;
        }

        let variant = dependency.variant_for(symbol, &variant_name)?;
        let definition_span =
            dependency.artifact_source_span(&symbol.source_path, variant.name_span)?;
        Some(DependencyVariantTarget {
            reference_span,
            package_name: dependency.artifact.package_name.clone(),
            manifest_path: dependency.manifest.manifest_path.clone(),
            source_path: symbol.source_path.clone(),
            enum_name: symbol.name.clone(),
            name: variant.name.clone(),
            detail: dependency_variant_detail(&symbol.name, variant),
            path: dependency.interface_path.clone(),
            definition_span,
        })
    }

    fn dependency_value_occurrence_in_module(
        &self,
        module: &ql_ast::Module,
        offset: usize,
    ) -> Option<DependencyValueOccurrence> {
        self.dependency_value_occurrences(module)
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
    }

    fn dependency_value_occurrences(
        &self,
        module: &ql_ast::Module,
    ) -> Vec<DependencyValueOccurrence> {
        let mut binding_scopes = vec![HashMap::new()];
        let mut iterable_scopes = vec![HashMap::new()];
        let mut value_scopes = vec![HashMap::new()];
        let mut occurrences = Vec::new();
        for item in &module.items {
            collect_dependency_value_occurrences_in_item(
                self,
                module,
                item,
                &mut binding_scopes,
                &mut iterable_scopes,
                &mut value_scopes,
                &mut occurrences,
            );
        }
        occurrences
    }

    fn dependency_struct_field_target_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyStructFieldTarget> {
        self.dependency_struct_field_occurrences(analysis.ast())
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
            .map(|occurrence| DependencyStructFieldTarget {
                reference_span: occurrence.reference_span,
                package_name: occurrence.package_name,
                manifest_path: occurrence.manifest_path,
                source_path: occurrence.source_path,
                struct_name: occurrence.struct_name,
                name: occurrence.name,
                detail: occurrence.detail,
                path: occurrence.path,
                definition_span: occurrence.definition_span,
            })
    }

    fn dependency_method_target_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyMethodTarget> {
        self.dependency_method_occurrences(analysis.ast())
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
            .map(|occurrence| DependencyMethodTarget {
                reference_span: occurrence.reference_span,
                package_name: occurrence.package_name,
                manifest_path: occurrence.manifest_path,
                source_path: occurrence.source_path,
                struct_name: occurrence.struct_name,
                name: occurrence.name,
                detail: occurrence.detail,
                path: occurrence.path,
                definition_span: occurrence.definition_span,
            })
    }

    fn dependency_struct_field_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyStructFieldTarget> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => return dependency_struct_field_target_in_broken_source(self, source, offset),
        };
        self.dependency_struct_field_occurrences(&module)
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
            .map(|occurrence| DependencyStructFieldTarget {
                reference_span: occurrence.reference_span,
                package_name: occurrence.package_name,
                manifest_path: occurrence.manifest_path,
                source_path: occurrence.source_path,
                struct_name: occurrence.struct_name,
                name: occurrence.name,
                detail: occurrence.detail,
                path: occurrence.path,
                definition_span: occurrence.definition_span,
            })
    }

    fn dependency_method_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyMethodTarget> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => return dependency_method_target_in_broken_source(self, source, offset),
        };
        self.dependency_method_occurrences(&module)
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
            .map(|occurrence| DependencyMethodTarget {
                reference_span: occurrence.reference_span,
                package_name: occurrence.package_name,
                manifest_path: occurrence.manifest_path,
                source_path: occurrence.source_path,
                struct_name: occurrence.struct_name,
                name: occurrence.name,
                detail: occurrence.detail,
                path: occurrence.path,
                definition_span: occurrence.definition_span,
            })
    }

    fn dependency_value_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyValueTarget> {
        let module = match parse_source(source) {
            Ok(module) => module,
            Err(_) => {
                let occurrence =
                    dependency_value_occurrence_in_broken_source(self, source, offset)?;
                return dependency_value_target_for_occurrence(self, &occurrence);
            }
        };
        if let Some(occurrence) = self.dependency_value_occurrence_in_module(&module, offset) {
            return dependency_value_target_for_occurrence(self, &occurrence);
        }

        let binding = self.dependency_value_root_binding_in_source_at(source, offset)?;
        Some(DependencyValueTarget {
            reference_span: Span::new(offset, offset),
            package_name: binding.package_name,
            manifest_path: binding.manifest_path,
            source_path: binding.source_path,
            struct_name: binding.struct_name,
            detail: binding.detail,
            path: binding.path,
            definition_span: binding.definition_span,
        })
    }

    fn dependency_struct_field_occurrences(
        &self,
        module: &ql_ast::Module,
    ) -> Vec<DependencyStructFieldOccurrence> {
        let mut occurrences = Vec::new();
        let mut scopes = vec![HashMap::new()];
        let mut iterable_scopes = vec![HashMap::new()];
        for item in &module.items {
            collect_dependency_struct_field_occurrences_in_item(
                self,
                module,
                item,
                &mut scopes,
                &mut iterable_scopes,
                &mut occurrences,
            );
        }
        occurrences
    }

    fn dependency_method_occurrences(
        &self,
        module: &ql_ast::Module,
    ) -> Vec<DependencyMethodOccurrence> {
        let mut occurrences = Vec::new();
        let mut scopes = vec![HashMap::new()];
        let mut iterable_scopes = vec![HashMap::new()];
        for item in &module.items {
            collect_dependency_method_occurrences_in_item(
                self,
                module,
                item,
                &mut scopes,
                &mut iterable_scopes,
                &mut occurrences,
            );
        }
        occurrences
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyVariantTarget {
    reference_span: Span,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    enum_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructFieldOccurrence {
    reference_span: Span,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug)]
struct DependencyValueBinding {
    kind: SymbolKind,
    local_name: String,
    definition_span: Span,
    definition_rename: DependencyValueDefinitionRename,
    dependency: DependencyStructBinding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DependencyValueDefinitionRename {
    Direct,
    StructShorthandField { field_name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyMethodOccurrence {
    reference_span: Span,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructFieldCompletionSite {
    root_name: String,
    excluded_field_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructFieldTarget {
    reference_span: Span,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyMethodTarget {
    reference_span: Span,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyValueTarget {
    reference_span: Span,
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructBinding {
    package_name: String,
    manifest_path: PathBuf,
    source_path: String,
    struct_name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
    fields: HashMap<String, DependencyStructResolvedField>,
    methods: HashMap<String, DependencyStructResolvedMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructResolvedField {
    name: String,
    detail: String,
    ty: String,
    definition_span: Span,
    type_definition: Option<DependencyDefinitionTarget>,
    question_type_definition: Option<DependencyDefinitionTarget>,
    iterable_element_type_definition: Option<DependencyDefinitionTarget>,
    question_iterable_element_type_definition: Option<DependencyDefinitionTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructResolvedMethod {
    name: String,
    source_path: String,
    detail: String,
    return_type: Option<String>,
    definition_span: Span,
    return_type_definition: Option<DependencyDefinitionTarget>,
    question_return_type_definition: Option<DependencyDefinitionTarget>,
    iterable_element_type_definition: Option<DependencyDefinitionTarget>,
    question_iterable_element_type_definition: Option<DependencyDefinitionTarget>,
}

type DependencyIterableScopes = Vec<HashMap<String, Option<DependencyStructBinding>>>;

fn dependency_value_target_for_occurrence(
    package: &PackageAnalysis,
    occurrence: &DependencyValueOccurrence,
) -> Option<DependencyValueTarget> {
    let binding = dependency_struct_binding_for_definition_target(
        package,
        &DependencyDefinitionTarget {
            package_name: occurrence.package_name.clone(),
            manifest_path: occurrence.manifest_path.clone(),
            source_path: occurrence.source_path.clone(),
            kind: SymbolKind::Struct,
            name: occurrence.struct_name.clone(),
            path: occurrence.path.clone(),
            span: occurrence.definition_span,
        },
    )?;
    Some(DependencyValueTarget {
        reference_span: occurrence.reference_span,
        package_name: occurrence.package_name.clone(),
        manifest_path: occurrence.manifest_path.clone(),
        source_path: occurrence.source_path.clone(),
        struct_name: occurrence.struct_name.clone(),
        detail: binding.detail,
        path: occurrence.path.clone(),
        definition_span: binding.definition_span,
    })
}

fn dependency_struct_bindings_match(
    left: &DependencyStructBinding,
    right: &DependencyStructBinding,
) -> bool {
    left.package_name == right.package_name
        && left.source_path == right.source_path
        && left.struct_name == right.struct_name
        && left.path == right.path
}

impl Analysis {
    pub fn ast(&self) -> &ql_ast::Module {
        &self.ast
    }

    pub fn hir(&self) -> &ql_hir::Module {
        &self.hir
    }

    pub fn mir(&self) -> &MirModule {
        &self.mir
    }

    pub fn resolution(&self) -> &ResolutionMap {
        &self.resolution
    }

    pub fn typeck(&self) -> &TypeckResult {
        &self.typeck
    }

    pub fn borrowck(&self) -> &BorrowckResult {
        &self.borrowck
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Return the runtime capabilities currently required by this module.
    ///
    /// This is a conservative source-ordered summary of the async/runtime surface
    /// already present in HIR, intended for future driver/codegen/runtime wiring.
    pub fn runtime_requirements(&self) -> &[RuntimeRequirement] {
        &self.runtime_requirements
    }

    pub fn has_errors(&self) -> bool {
        !self.diagnostics().is_empty()
    }

    pub fn expr_ty(&self, expr_id: ExprId) -> Option<&Ty> {
        self.typeck.expr_ty(expr_id)
    }

    pub fn pattern_ty(&self, pattern_id: PatternId) -> Option<&Ty> {
        self.typeck.pattern_ty(pattern_id)
    }

    pub fn local_ty(&self, local_id: LocalId) -> Option<&Ty> {
        self.typeck.local_ty(local_id)
    }

    /// Return the smallest indexed semantic symbol that covers `offset`.
    ///
    /// This currently stays conservative on purpose:
    /// - expression queries cover root bindings plus struct fields, unique method members, and
    ///   enum variant tokens, but still do not model deeper member chains or module-path semantics
    /// - builtin types can hover but have no source definition span
    /// - import aliases are now source-backed within the current file, and local aliases that
    ///   point at local enum items can forward variant-token queries, but deeper module graphs
    ///   still are not resolved beyond the imported root binding
    pub fn symbol_at(&self, offset: usize) -> Option<HoverInfo> {
        self.index.symbol_at(offset)
    }

    /// Return hover-ready semantic data for the symbol covering `offset`.
    pub fn hover_at(&self, offset: usize) -> Option<HoverInfo> {
        self.symbol_at(offset)
    }

    /// Return the source-backed import binding covering `offset`.
    pub fn import_binding_at(&self, offset: usize) -> Option<(ImportBinding, Span)> {
        self.index.import_binding_at(offset)
    }

    pub fn type_import_binding_at(&self, offset: usize) -> Option<ImportBinding> {
        self.index.type_import_binding_at(offset)
    }

    /// Return async semantic context for `await` / `spawn` / `for await` at `offset`.
    pub fn async_context_at(&self, offset: usize) -> Option<AsyncContextInfo> {
        self.index.async_context_at(offset)
    }

    /// Return loop-control semantic context for `break` / `continue` at `offset`.
    pub fn loop_control_context_at(&self, offset: usize) -> Option<LoopControlContextInfo> {
        self.index.loop_control_context_at(offset)
    }

    /// Return the definition site for the symbol covering `offset`, when the target lives in source.
    pub fn definition_at(&self, offset: usize) -> Option<DefinitionTarget> {
        self.index.definition_at(offset)
    }

    /// Return the explicit type-definition target covering `offset`, when the target lives in source.
    pub fn type_definition_at(&self, offset: usize) -> Option<DefinitionTarget> {
        self.index.type_definition_at(offset)
    }

    /// Return same-file implementation sites for the symbol covering `offset`.
    ///
    /// This currently stays conservative on purpose:
    /// - type implementation queries return same-file `impl` / `extend` block spans
    /// - trait implementation queries return same-file `impl Trait for ...` block spans
    /// - trait-method implementation queries return matching same-file impl method definition spans
    /// - method call sites that already resolve to concrete impl/extend methods still use
    ///   `definition_at`
    pub fn implementations_at(&self, offset: usize) -> Option<Vec<ImplementationTarget>> {
        let definition = self.definition_at(offset)?;

        let mut targets = match definition.kind {
            SymbolKind::Struct | SymbolKind::Enum => {
                let item_id = self.same_file_item_id_for_definition(&definition)?;
                self.implementation_targets_for_type_item(item_id)
            }
            SymbolKind::Trait => {
                let item_id = self.same_file_item_id_for_definition(&definition)?;
                self.implementation_targets_for_trait_item(item_id)
            }
            SymbolKind::Method => {
                let (trait_item_id, method_name) =
                    self.same_file_trait_method_for_definition_span(definition.span)?;
                self.implementation_targets_for_trait_method(trait_item_id, &method_name)
            }
            _ => Vec::new(),
        };

        if targets.is_empty() {
            return None;
        }

        targets.sort_by_key(|target| (target.span.start, target.span.end));
        targets.dedup_by_key(|target| (target.span.start, target.span.end));
        Some(targets)
    }

    /// Return every indexed occurrence for the symbol covering `offset` within the current file.
    pub fn references_at(&self, offset: usize) -> Option<Vec<ReferenceTarget>> {
        self.index.references_at(offset)
    }

    /// Return rename metadata when the symbol under `offset` is safe for same-file renaming.
    pub fn prepare_rename_at(&self, offset: usize) -> Option<RenameTarget> {
        self.index.prepare_rename_at(offset)
    }

    /// Return same-file rename edits for the symbol covering `offset`.
    pub fn rename_at(
        &self,
        offset: usize,
        new_name: &str,
    ) -> Result<Option<RenameResult>, RenameError> {
        self.index.rename_at(offset, new_name)
    }

    /// Return visible completion candidates at `offset`.
    ///
    /// This currently stays conservative on purpose:
    /// - only same-file completion is supported
    /// - lexical-scope completion currently covers value and type positions already represented in HIR
    /// - member completion currently covers already-parsed member tokens whose receiver type is
    ///   stable, plus same-file parsed enum variant paths, including local import aliases that
    ///   point at local enum items
    /// - parse-tolerant same-file struct field-label completion covers direct and deeper
    ///   struct-like literal/pattern paths when the root resolves to a local struct item
    /// - ambiguous member surfaces, parse-error tolerant completion, and cross-file project indexing
    ///   are still intentionally deferred
    pub fn completions_at(&self, offset: usize) -> Option<Vec<CompletionItem>> {
        self.index
            .semantic_completions_at(offset)
            .or_else(|| self.local_struct_field_completions_at(offset))
            .or_else(|| self.index.lexical_completions_at(offset))
    }

    /// Return source-backed semantic-token occurrences for the current file.
    ///
    /// This intentionally reuses the same conservative query surface as hover/definition/references:
    /// only tokens with stable source-backed semantic identity are emitted.
    pub fn semantic_tokens(&self) -> Vec<SemanticTokenOccurrence> {
        self.index.semantic_tokens()
    }

    /// Return source-backed document outline declarations for the current file.
    ///
    /// This stays conservative and only exports document-level declarations with stable source
    /// identity: top-level items plus member declarations already represented in the query index.
    pub fn document_symbols(&self) -> Vec<DocumentSymbolTarget> {
        self.index.document_symbols()
    }

    fn local_struct_field_completions_at(&self, offset: usize) -> Option<Vec<CompletionItem>> {
        let site = local_struct_field_completion_site(&self.ast, offset)?;
        self.index
            .local_struct_field_completion_items_for_root_name(
                &site.root_name,
                &site.excluded_field_names,
            )
    }

    pub fn render_diagnostics(&self, path: &Path, source: &str) -> String {
        render_diagnostics(path, source, self.diagnostics())
    }

    pub fn render_mir(&self) -> String {
        render_mir_module(&self.mir, &self.hir)
    }

    pub fn render_borrowck(&self) -> String {
        render_borrowck_result(&self.borrowck, &self.mir)
    }

    fn same_file_item_id_for_definition(&self, definition: &DefinitionTarget) -> Option<ItemId> {
        self.hir.items.iter().copied().find(|&item_id| {
            match (&self.hir.item(item_id).kind, definition.kind) {
                (HirItemKind::Struct(struct_decl), SymbolKind::Struct) => {
                    struct_decl.name_span == definition.span
                }
                (HirItemKind::Enum(enum_decl), SymbolKind::Enum) => {
                    enum_decl.name_span == definition.span
                }
                (HirItemKind::Trait(trait_decl), SymbolKind::Trait) => {
                    trait_decl.name_span == definition.span
                }
                _ => false,
            }
        })
    }

    fn same_file_trait_method_for_definition_span(&self, span: Span) -> Option<(ItemId, String)> {
        self.hir.items.iter().copied().find_map(|item_id| {
            let HirItemKind::Trait(trait_decl) = &self.hir.item(item_id).kind else {
                return None;
            };
            trait_decl
                .methods
                .iter()
                .find(|method| method.name_span == span)
                .map(|method| (item_id, method.name.clone()))
        })
    }

    fn implementation_targets_for_type_item(&self, item_id: ItemId) -> Vec<ImplementationTarget> {
        let mut targets = Vec::new();

        for &candidate_item_id in &self.hir.items {
            match &self.hir.item(candidate_item_id).kind {
                HirItemKind::Impl(impl_block)
                    if self.type_item_for_type_id(impl_block.target) == Some(item_id) =>
                {
                    targets.push(ImplementationTarget {
                        span: self.hir.item(candidate_item_id).span,
                    });
                }
                HirItemKind::Extend(extend_block)
                    if self.type_item_for_type_id(extend_block.target) == Some(item_id) =>
                {
                    targets.push(ImplementationTarget {
                        span: self.hir.item(candidate_item_id).span,
                    });
                }
                _ => {}
            }
        }

        targets
    }

    fn implementation_targets_for_trait_item(&self, item_id: ItemId) -> Vec<ImplementationTarget> {
        let mut targets = Vec::new();

        for &candidate_item_id in &self.hir.items {
            let HirItemKind::Impl(impl_block) = &self.hir.item(candidate_item_id).kind else {
                continue;
            };
            if impl_block
                .trait_ty
                .and_then(|trait_ty| self.type_item_for_type_id(trait_ty))
                == Some(item_id)
            {
                targets.push(ImplementationTarget {
                    span: self.hir.item(candidate_item_id).span,
                });
            }
        }

        targets
    }

    fn implementation_targets_for_trait_method(
        &self,
        trait_item_id: ItemId,
        method_name: &str,
    ) -> Vec<ImplementationTarget> {
        let mut targets = Vec::new();

        for &candidate_item_id in &self.hir.items {
            let HirItemKind::Impl(impl_block) = &self.hir.item(candidate_item_id).kind else {
                continue;
            };
            if impl_block
                .trait_ty
                .and_then(|trait_ty| self.type_item_for_type_id(trait_ty))
                != Some(trait_item_id)
            {
                continue;
            }
            targets.extend(
                impl_block
                    .methods
                    .iter()
                    .filter(|method| method.name == method_name)
                    .map(|method| ImplementationTarget {
                        span: method.name_span,
                    }),
            );
        }

        targets
    }

    fn type_item_for_type_id(&self, type_id: ql_hir::TypeId) -> Option<ItemId> {
        match self.resolution.type_resolution(type_id) {
            Some(TypeResolution::Item(item_id)) => Some(*item_id),
            _ => None,
        }
    }
}

/// Analyze one source string. Parse failures are returned as diagnostics directly.
/// Resolution and type diagnostics are stored on the returned [`Analysis`] even when errors exist.
pub fn analyze_source(source: &str) -> Result<Analysis, Vec<Diagnostic>> {
    let ast = parse_source(source).map_err(parse_errors_to_diagnostics)?;
    let hir = lower_module(&ast);
    let resolution = resolve_module(&hir);
    let typeck = analyze_types(&hir, &resolution);
    let mir = lower_mir_with_typeck(&hir, &resolution, &typeck);
    let borrowck = analyze_borrowck(&hir, &resolution, &typeck, &mir);
    let index = QueryIndex::build(source, &hir, &resolution, &typeck);
    let runtime_requirements = runtime::collect_runtime_requirements(source, &hir);
    let mut diagnostics = resolution.diagnostics.clone();
    diagnostics.extend(typeck.diagnostics().iter().cloned());
    diagnostics.extend(borrowck.diagnostics().iter().cloned());

    Ok(Analysis {
        ast,
        hir,
        mir,
        resolution,
        typeck,
        borrowck,
        index,
        runtime_requirements,
        diagnostics,
    })
}

pub fn analyze_package(path: &Path) -> Result<PackageAnalysis, PackageAnalysisError> {
    let manifest = load_project_manifest(path).map_err(PackageAnalysisError::Project)?;
    let files = collect_package_sources(&manifest).map_err(PackageAnalysisError::Project)?;

    let mut modules = Vec::with_capacity(files.len());
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| PackageAnalysisError::Read {
            path: file.clone(),
            error,
        })?;
        let analysis = analyze_source(&source).map_err(|diagnostics| {
            PackageAnalysisError::SourceDiagnostics {
                path: file.clone(),
                source: source.clone(),
                diagnostics,
            }
        })?;
        if analysis.has_errors() {
            return Err(PackageAnalysisError::SourceDiagnostics {
                path: file,
                source,
                diagnostics: analysis.diagnostics().to_vec(),
            });
        }
        modules.push(PackageModuleAnalysis {
            path: file,
            analysis,
        });
    }

    let dependencies = load_package_dependencies(&manifest)?;

    Ok(PackageAnalysis {
        manifest,
        modules,
        dependencies,
    })
}

pub fn analyze_package_dependencies(path: &Path) -> Result<PackageAnalysis, PackageAnalysisError> {
    let manifest = load_project_manifest(path).map_err(PackageAnalysisError::Project)?;
    let dependencies = load_package_dependencies(&manifest)?;
    Ok(PackageAnalysis {
        manifest,
        modules: Vec::new(),
        dependencies,
    })
}

pub fn analyze_available_package_dependencies(
    path: &Path,
) -> Result<Vec<DependencyInterface>, PackageAnalysisError> {
    let manifest = load_project_manifest(path).map_err(PackageAnalysisError::Project)?;
    Ok(load_available_package_dependencies(&manifest))
}

pub fn analyze_package_with_available_dependencies(
    path: &Path,
) -> Result<PackageAnalysis, PackageAnalysisError> {
    let manifest = load_project_manifest(path).map_err(PackageAnalysisError::Project)?;
    let dependencies = load_available_package_dependencies(&manifest);
    Ok(PackageAnalysis {
        manifest,
        modules: Vec::new(),
        dependencies,
    })
}

fn load_package_dependencies(
    manifest: &ProjectManifest,
) -> Result<Vec<DependencyInterface>, PackageAnalysisError> {
    let dependency_manifests =
        load_reference_manifests(manifest).map_err(PackageAnalysisError::Project)?;
    let mut dependencies = Vec::with_capacity(dependency_manifests.len());
    for dependency_manifest in dependency_manifests {
        let interface_path =
            default_interface_path(&dependency_manifest).map_err(PackageAnalysisError::Project)?;
        if !interface_path.is_file() {
            let package_name = dependency_manifest
                .package
                .as_ref()
                .map(|package| package.name.clone())
                .unwrap_or_else(|| "<unknown>".to_owned());
            return Err(PackageAnalysisError::InterfaceNotFound {
                package_name,
                path: interface_path,
            });
        }
        let artifact = load_interface_artifact(&interface_path).map_err(|error| match error {
            InterfaceError::Read { path, error } => PackageAnalysisError::Read { path, error },
            InterfaceError::Parse { path, message } => {
                PackageAnalysisError::InterfaceParse { path, message }
            }
        })?;
        let symbols = index_dependency_symbols(&artifact);
        dependencies.push(DependencyInterface {
            manifest: dependency_manifest,
            interface_path,
            artifact,
            symbols,
        });
    }
    Ok(dependencies)
}

fn load_available_package_dependencies(manifest: &ProjectManifest) -> Vec<DependencyInterface> {
    let manifest_dir = manifest
        .manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let mut dependencies = Vec::with_capacity(manifest.references.packages.len());

    for package in &manifest.references.packages {
        let dependency_manifest = match load_project_manifest(&manifest_dir.join(package)) {
            Ok(manifest) => manifest,
            Err(_) => continue,
        };
        let interface_path = match default_interface_path(&dependency_manifest) {
            Ok(path) => path,
            Err(_) => continue,
        };
        if !interface_path.is_file() {
            continue;
        }
        let artifact = match load_interface_artifact(&interface_path) {
            Ok(artifact) => artifact,
            Err(_) => continue,
        };
        let symbols = index_dependency_symbols(&artifact);
        dependencies.push(DependencyInterface {
            manifest: dependency_manifest,
            interface_path,
            artifact,
            symbols,
        });
    }

    dependencies
}

fn normalized_relative_source_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalized_module_source_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn package_module_source_path(package: &PackageAnalysis, module_path: &Path) -> Option<String> {
    let package_root = package.manifest.manifest_path.parent()?;
    let relative_path = module_path.strip_prefix(package_root).ok()?;
    Some(normalized_relative_source_path(relative_path))
}

fn package_module_matches_source_path(
    package: &PackageAnalysis,
    module_path: &Path,
    source_path: &str,
) -> bool {
    package_module_source_path(package, module_path)
        .is_some_and(|relative_path| relative_path == normalized_module_source_path(source_path))
}

fn package_module_for_source_path<'a>(
    package: &'a PackageAnalysis,
    source_path: &str,
) -> Option<&'a PackageModuleAnalysis> {
    package
        .modules
        .iter()
        .find(|module| package_module_matches_source_path(package, module.path(), source_path))
}

fn enum_variant_completions_in_module_source(
    module_source: &str,
    enum_name: &str,
) -> Option<Vec<CompletionItem>> {
    let module = parse_source(module_source).ok()?;
    let enum_decl = module.items.iter().find_map(|item| match &item.kind {
        AstItemKind::Enum(enum_decl)
            if is_public(&enum_decl.visibility) && enum_decl.name == enum_name =>
        {
            Some(enum_decl)
        }
        _ => None,
    })?;

    let mut items = enum_decl
        .variants
        .iter()
        .map(|variant| CompletionItem {
            label: variant.name.clone(),
            insert_text: variant.name.clone(),
            kind: SymbolKind::Variant,
            detail: dependency_variant_detail(&enum_decl.name, variant),
            ty: Some(enum_decl.name.clone()),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.detail.cmp(&right.detail))
    });
    Some(items)
}

fn dependency_struct_completion_target(
    dependency: &DependencyInterface,
    symbol: &DependencySymbol,
) -> Option<DependencyStructCompletionTarget> {
    (symbol.kind == SymbolKind::Struct).then(|| DependencyStructCompletionTarget {
        package_name: dependency.artifact.package_name.clone(),
        manifest_path: dependency.manifest.manifest_path.clone(),
        source_path: symbol.source_path.clone(),
        struct_name: symbol.name.clone(),
        path: dependency.interface_path.clone(),
    })
}

fn dependency_member_completion_target_in_source_at(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructCompletionTarget> {
    let module = parse_source(source).ok()?;
    let target_kind = match kind {
        DependencyMemberCompletionKind::Field => DependencyMemberCompletionKind::FieldReceiver,
        DependencyMemberCompletionKind::Method => DependencyMemberCompletionKind::MethodReceiver,
        other => other,
    };
    let binding =
        dependency_member_completion_binding(package, &module, source, offset, target_kind)?;
    Some(DependencyStructCompletionTarget {
        package_name: binding.package_name,
        manifest_path: binding.manifest_path,
        source_path: binding.source_path,
        struct_name: binding.struct_name,
        path: binding.path,
    })
}

fn dependency_member_completion_target_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructCompletionTarget> {
    let site = dependency_member_site_in_broken_source(source, offset)?;
    let expected_kind = match kind {
        DependencyMemberCompletionKind::Field | DependencyMemberCompletionKind::FieldReceiver => {
            BrokenSourceValueSegmentKind::Field
        }
        DependencyMemberCompletionKind::Method | DependencyMemberCompletionKind::MethodReceiver => {
            BrokenSourceValueSegmentKind::Method
        }
        DependencyMemberCompletionKind::ValueType => return None,
    };
    if site.member_kind != expected_kind {
        return None;
    }
    let binding = dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
    Some(DependencyStructCompletionTarget {
        package_name: binding.package_name,
        manifest_path: binding.manifest_path,
        source_path: binding.source_path,
        struct_name: binding.struct_name,
        path: binding.path,
    })
}

fn public_struct_completion_binding(
    package: &PackageAnalysis,
    source_path: &str,
    struct_name: &str,
) -> Option<DependencyStructBinding> {
    let module = package_module_for_source_path(package, source_path)?;
    let module_source = fs::read_to_string(module.path())
        .ok()?
        .replace("\r\n", "\n");
    public_struct_completion_binding_in_source(package, source_path, &module_source, struct_name)
}

fn public_type_symbol_in_module(
    module: &ql_ast::Module,
    type_name: &str,
) -> Vec<(SymbolKind, Span, String)> {
    module
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            AstItemKind::Struct(struct_decl)
                if is_public(&struct_decl.visibility) && struct_decl.name == type_name =>
            {
                Some((
                    SymbolKind::Struct,
                    struct_decl.name_span,
                    struct_decl.name.clone(),
                ))
            }
            AstItemKind::Enum(enum_decl)
                if is_public(&enum_decl.visibility) && enum_decl.name == type_name =>
            {
                Some((
                    SymbolKind::Enum,
                    enum_decl.name_span,
                    enum_decl.name.clone(),
                ))
            }
            AstItemKind::Trait(trait_decl)
                if is_public(&trait_decl.visibility) && trait_decl.name == type_name =>
            {
                Some((
                    SymbolKind::Trait,
                    trait_decl.name_span,
                    trait_decl.name.clone(),
                ))
            }
            AstItemKind::TypeAlias(type_alias)
                if is_public(&type_alias.visibility) && type_alias.name == type_name =>
            {
                Some((
                    SymbolKind::TypeAlias,
                    type_alias.name_span,
                    type_alias.name.clone(),
                ))
            }
            _ => None,
        })
        .collect()
}

fn package_public_type_target_for_type_expr_with_override(
    package: &PackageAnalysis,
    source_override: Option<(&str, &ql_ast::Module)>,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyDefinitionTarget> {
    let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
        return None;
    };
    let [type_name] = path.segments.as_slice() else {
        return None;
    };

    let mut matches = Vec::<(SymbolKind, Span, String, String)>::new();
    if let Some((override_source_path, override_module)) = source_override {
        matches.extend(
            public_type_symbol_in_module(override_module, type_name)
                .into_iter()
                .map(|(kind, span, name)| {
                    (
                        kind,
                        span,
                        normalized_module_source_path(override_source_path),
                        name,
                    )
                }),
        );
    }

    for module in &package.modules {
        let Some(module_source_path) = package_module_source_path(package, module.path()) else {
            continue;
        };
        if source_override.is_some_and(|(override_source_path, _)| {
            normalized_module_source_path(override_source_path) == module_source_path
        }) {
            continue;
        }
        matches.extend(
            public_type_symbol_in_module(module.analysis().ast(), type_name)
                .into_iter()
                .map(|(kind, span, name)| (kind, span, module_source_path.clone(), name)),
        );
    }

    if matches.len() != 1 {
        return None;
    }
    let (kind, span, source_path, name) = matches.pop()?;
    Some(DependencyDefinitionTarget {
        package_name: package.manifest.package.as_ref()?.name.clone(),
        manifest_path: package.manifest.manifest_path.clone(),
        source_path,
        kind,
        name,
        path: default_interface_path(&package.manifest).ok()?,
        span,
    })
}

fn package_question_inner_type_target_for_type_expr_with_override(
    package: &PackageAnalysis,
    source_override: Option<(&str, &ql_ast::Module)>,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyDefinitionTarget> {
    let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    let [type_name] = path.segments.as_slice() else {
        return None;
    };
    let inner = match (type_name.as_str(), args.as_slice()) {
        ("Option", [inner]) => inner,
        ("Result", [inner, ..]) => inner,
        _ => return None,
    };
    package_public_type_target_for_type_expr_with_override(package, source_override, inner).or_else(
        || {
            package_question_inner_type_target_for_type_expr_with_override(
                package,
                source_override,
                inner,
            )
        },
    )
}

fn package_common_type_target_for_type_exprs_with_override(
    package: &PackageAnalysis,
    source_override: Option<(&str, &ql_ast::Module)>,
    items: &[ql_ast::TypeExpr],
) -> Option<DependencyDefinitionTarget> {
    let mut items = items.iter();
    let first = package_public_type_target_for_type_expr_with_override(
        package,
        source_override,
        items.next()?,
    )?;
    for item in items {
        let target =
            package_public_type_target_for_type_expr_with_override(package, source_override, item)?;
        if target != first {
            return None;
        }
    }
    Some(first)
}

fn package_iterable_element_type_target_for_type_expr_with_override(
    package: &PackageAnalysis,
    source_override: Option<(&str, &ql_ast::Module)>,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyDefinitionTarget> {
    match &ty.kind {
        ql_ast::TypeExprKind::Array { element, .. } => {
            package_public_type_target_for_type_expr_with_override(
                package,
                source_override,
                element,
            )
        }
        ql_ast::TypeExprKind::Tuple(items) => {
            package_common_type_target_for_type_exprs_with_override(package, source_override, items)
        }
        _ => None,
    }
}

fn package_question_inner_iterable_element_type_target_for_type_expr_with_override(
    package: &PackageAnalysis,
    source_override: Option<(&str, &ql_ast::Module)>,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyDefinitionTarget> {
    let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    let [type_name] = path.segments.as_slice() else {
        return None;
    };
    let inner = match (type_name.as_str(), args.as_slice()) {
        ("Option", [inner]) => inner,
        ("Result", [inner, ..]) => inner,
        _ => return None,
    };
    package_iterable_element_type_target_for_type_expr_with_override(
        package,
        source_override,
        inner,
    )
    .or_else(|| {
        package_question_inner_iterable_element_type_target_for_type_expr_with_override(
            package,
            source_override,
            inner,
        )
    })
}

fn public_struct_completion_binding_in_source(
    package: &PackageAnalysis,
    source_path: &str,
    module_source: &str,
    struct_name: &str,
) -> Option<DependencyStructBinding> {
    package_module_for_source_path(package, source_path)?;
    let module = parse_source(module_source).ok()?;
    let (struct_decl, detail) = module.items.iter().find_map(|item| match &item.kind {
        AstItemKind::Struct(struct_decl)
            if is_public(&struct_decl.visibility) && struct_decl.name == struct_name =>
        {
            Some((
                struct_decl,
                interface_detail_text(module_source, item.span, &struct_decl.name),
            ))
        }
        _ => None,
    })?;
    let fields = struct_decl
        .fields
        .iter()
        .map(|field| {
            (
                field.name.clone(),
                DependencyStructResolvedField {
                    name: field.name.clone(),
                    detail: dependency_struct_field_detail(field),
                    ty: render_dependency_type_expr(&field.ty),
                    definition_span: field.name_span,
                    type_definition: package_public_type_target_for_type_expr_with_override(
                        package,
                        Some((source_path, &module)),
                        &field.ty,
                    ),
                    question_type_definition:
                        package_question_inner_type_target_for_type_expr_with_override(
                            package,
                            Some((source_path, &module)),
                            &field.ty,
                        ),
                    iterable_element_type_definition:
                        package_iterable_element_type_target_for_type_expr_with_override(
                            package,
                            Some((source_path, &module)),
                            &field.ty,
                        ),
                    question_iterable_element_type_definition:
                        package_question_inner_iterable_element_type_target_for_type_expr_with_override(
                            package,
                            Some((source_path, &module)),
                            &field.ty,
                        ),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    Some(DependencyStructBinding {
        package_name: package.manifest.package.as_ref()?.name.clone(),
        manifest_path: package.manifest.manifest_path.clone(),
        source_path: source_path.to_owned(),
        struct_name: struct_name.to_owned(),
        detail,
        path: default_interface_path(&package.manifest).ok()?,
        definition_span: struct_decl.name_span,
        fields,
        methods: public_struct_methods_for_source_path(
            package,
            struct_name,
            Some((source_path, module_source)),
        ),
    })
}

fn public_struct_methods_for_source_path(
    package: &PackageAnalysis,
    struct_name: &str,
    source_override: Option<(&str, &str)>,
) -> HashMap<String, DependencyStructResolvedMethod> {
    let mut impl_candidates: HashMap<String, Vec<DependencyStructResolvedMethod>> = HashMap::new();
    let mut extend_candidates: HashMap<String, Vec<DependencyStructResolvedMethod>> =
        HashMap::new();

    for module in &package.modules {
        let Some(module_source_path) = package_module_source_path(package, module.path()) else {
            continue;
        };
        let (module_source, module_ast) = if let Some((override_source_path, override_source)) =
            source_override
            && normalized_module_source_path(override_source_path) == module_source_path
        {
            if let Ok(parsed_module) = parse_source(override_source) {
                (override_source.replace("\r\n", "\n"), parsed_module)
            } else {
                let Ok(module_source) = fs::read_to_string(module.path()) else {
                    continue;
                };
                (
                    module_source.replace("\r\n", "\n"),
                    module.analysis().ast().clone(),
                )
            }
        } else {
            let Ok(module_source) = fs::read_to_string(module.path()) else {
                continue;
            };
            (
                module_source.replace("\r\n", "\n"),
                module.analysis().ast().clone(),
            )
        };
        for item in &module_ast.items {
            match &item.kind {
                AstItemKind::Impl(impl_block)
                    if dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
                {
                    for method in impl_block
                        .methods
                        .iter()
                        .filter(|method| is_public(&method.visibility))
                    {
                        impl_candidates
                            .entry(method.name.clone())
                            .or_default()
                            .push(DependencyStructResolvedMethod {
                                name: method.name.clone(),
                                source_path: module_source_path.clone(),
                                detail: source_function_detail_text(
                                    &module_source,
                                    method.span,
                                    &method.name,
                                ),
                                return_type: method
                                    .return_type
                                    .as_ref()
                                    .map(render_dependency_type_expr),
                                definition_span: method.name_span,
                                return_type_definition: method.return_type.as_ref().and_then(|ty| {
                                    package_public_type_target_for_type_expr_with_override(
                                        package,
                                        Some((module_source_path.as_str(), &module_ast)),
                                        ty,
                                    )
                                }),
                                question_return_type_definition: method
                                    .return_type
                                    .as_ref()
                                    .and_then(|ty| {
                                        package_question_inner_type_target_for_type_expr_with_override(
                                            package,
                                            Some((module_source_path.as_str(), &module_ast)),
                                            ty,
                                        )
                                    }),
                                iterable_element_type_definition: method
                                    .return_type
                                    .as_ref()
                                    .and_then(|ty| {
                                        package_iterable_element_type_target_for_type_expr_with_override(
                                            package,
                                            Some((module_source_path.as_str(), &module_ast)),
                                            ty,
                                        )
                                    }),
                                question_iterable_element_type_definition: method
                                    .return_type
                                    .as_ref()
                                    .and_then(|ty| {
                                        package_question_inner_iterable_element_type_target_for_type_expr_with_override(
                                            package,
                                            Some((module_source_path.as_str(), &module_ast)),
                                            ty,
                                        )
                                    }),
                            });
                    }
                }
                AstItemKind::Extend(extend_block)
                    if dependency_type_expr_targets_struct(&extend_block.target, struct_name) =>
                {
                    for method in extend_block
                        .methods
                        .iter()
                        .filter(|method| is_public(&method.visibility))
                    {
                        extend_candidates
                            .entry(method.name.clone())
                            .or_default()
                            .push(DependencyStructResolvedMethod {
                                name: method.name.clone(),
                                source_path: module_source_path.clone(),
                                detail: source_function_detail_text(
                                    &module_source,
                                    method.span,
                                    &method.name,
                                ),
                                return_type: method
                                    .return_type
                                    .as_ref()
                                    .map(render_dependency_type_expr),
                                definition_span: method.name_span,
                                return_type_definition: method.return_type.as_ref().and_then(|ty| {
                                    package_public_type_target_for_type_expr_with_override(
                                        package,
                                        Some((module_source_path.as_str(), &module_ast)),
                                        ty,
                                    )
                                }),
                                question_return_type_definition: method
                                    .return_type
                                    .as_ref()
                                    .and_then(|ty| {
                                        package_question_inner_type_target_for_type_expr_with_override(
                                            package,
                                            Some((module_source_path.as_str(), &module_ast)),
                                            ty,
                                        )
                                    }),
                                iterable_element_type_definition: method
                                    .return_type
                                    .as_ref()
                                    .and_then(|ty| {
                                        package_iterable_element_type_target_for_type_expr_with_override(
                                            package,
                                            Some((module_source_path.as_str(), &module_ast)),
                                            ty,
                                        )
                                    }),
                                question_iterable_element_type_definition: method
                                    .return_type
                                    .as_ref()
                                    .and_then(|ty| {
                                        package_question_inner_iterable_element_type_target_for_type_expr_with_override(
                                            package,
                                            Some((module_source_path.as_str(), &module_ast)),
                                            ty,
                                        )
                                    }),
                            });
                    }
                }
                _ => {}
            }
        }
    }

    let mut methods = HashMap::new();
    for (name, candidates) in impl_candidates {
        if candidates.len() == 1 {
            methods.insert(name, candidates.into_iter().next().unwrap());
        }
    }
    for (name, candidates) in extend_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    methods
}

fn index_dependency_symbols(artifact: &InterfaceArtifact) -> Vec<DependencySymbol> {
    let mut symbols = Vec::new();
    for module in &artifact.modules {
        index_interface_module_symbols(&artifact.package_name, module, &mut symbols);
    }
    symbols
}

fn index_interface_module_symbols(
    package_name: &str,
    module: &InterfaceModule,
    symbols: &mut Vec<DependencySymbol>,
) {
    for item in &module.syntax.items {
        index_interface_item_symbols(package_name, module, item, symbols);
    }
}

fn index_interface_item_symbols(
    package_name: &str,
    module: &InterfaceModule,
    item: &AstItem,
    symbols: &mut Vec<DependencySymbol>,
) {
    match &item.kind {
        AstItemKind::Function(function) if is_public(&function.visibility) => {
            push_dependency_symbol(
                package_name,
                &module.source_path,
                SymbolKind::Function,
                &function.name,
                function.span,
                interface_detail_text(&module.contents, function.span, &function.name),
                symbols,
            )
        }
        AstItemKind::Const(global) if is_public(&global.visibility) => push_dependency_symbol(
            package_name,
            &module.source_path,
            SymbolKind::Const,
            &global.name,
            item.span,
            interface_detail_text(&module.contents, item.span, &global.name),
            symbols,
        ),
        AstItemKind::Static(global) if is_public(&global.visibility) => push_dependency_symbol(
            package_name,
            &module.source_path,
            SymbolKind::Static,
            &global.name,
            item.span,
            interface_detail_text(&module.contents, item.span, &global.name),
            symbols,
        ),
        AstItemKind::Struct(struct_decl) if is_public(&struct_decl.visibility) => {
            push_dependency_symbol(
                package_name,
                &module.source_path,
                SymbolKind::Struct,
                &struct_decl.name,
                item.span,
                interface_detail_text(&module.contents, item.span, &struct_decl.name),
                symbols,
            );
        }
        AstItemKind::Enum(enum_decl) if is_public(&enum_decl.visibility) => {
            push_dependency_symbol(
                package_name,
                &module.source_path,
                SymbolKind::Enum,
                &enum_decl.name,
                item.span,
                interface_detail_text(&module.contents, item.span, &enum_decl.name),
                symbols,
            );
        }
        AstItemKind::Trait(trait_decl) if is_public(&trait_decl.visibility) => {
            push_dependency_symbol(
                package_name,
                &module.source_path,
                SymbolKind::Trait,
                &trait_decl.name,
                item.span,
                interface_detail_text(&module.contents, item.span, &trait_decl.name),
                symbols,
            );
            for method in &trait_decl.methods {
                push_dependency_symbol(
                    package_name,
                    &module.source_path,
                    SymbolKind::Method,
                    &method.name,
                    method.span,
                    interface_detail_text(&module.contents, method.span, &method.name),
                    symbols,
                );
            }
        }
        AstItemKind::Impl(impl_block) => {
            for method in impl_block
                .methods
                .iter()
                .filter(|method| is_public(&method.visibility))
            {
                push_dependency_symbol(
                    package_name,
                    &module.source_path,
                    SymbolKind::Method,
                    &method.name,
                    method.span,
                    interface_detail_text(&module.contents, method.span, &method.name),
                    symbols,
                );
            }
        }
        AstItemKind::Extend(extend_block) => {
            for method in extend_block
                .methods
                .iter()
                .filter(|method| is_public(&method.visibility))
            {
                push_dependency_symbol(
                    package_name,
                    &module.source_path,
                    SymbolKind::Method,
                    &method.name,
                    method.span,
                    interface_detail_text(&module.contents, method.span, &method.name),
                    symbols,
                );
            }
        }
        AstItemKind::TypeAlias(type_alias) if is_public(&type_alias.visibility) => {
            push_dependency_symbol(
                package_name,
                &module.source_path,
                SymbolKind::TypeAlias,
                &type_alias.name,
                item.span,
                interface_detail_text(&module.contents, item.span, &type_alias.name),
                symbols,
            );
        }
        AstItemKind::ExternBlock(extern_block) if is_public(&extern_block.visibility) => {
            for function in &extern_block.functions {
                push_dependency_symbol(
                    package_name,
                    &module.source_path,
                    SymbolKind::Function,
                    &function.name,
                    function.span,
                    interface_detail_text(&module.contents, function.span, &function.name),
                    symbols,
                );
            }
        }
        _ => {}
    }
}

fn push_dependency_symbol(
    package_name: &str,
    source_path: &str,
    kind: SymbolKind,
    name: &str,
    span: Span,
    detail: String,
    symbols: &mut Vec<DependencySymbol>,
) {
    symbols.push(DependencySymbol {
        package_name: package_name.to_owned(),
        source_path: source_path.to_owned(),
        kind,
        name: name.to_owned(),
        detail,
        span,
    });
}

fn interface_detail_text(source: &str, span: Span, fallback_name: &str) -> String {
    let detail = source
        .get(span.start..span.end)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback_name);
    detail
        .strip_prefix("pub ")
        .unwrap_or(detail)
        .trim()
        .to_owned()
}

fn source_function_detail_text(source: &str, span: Span, fallback_name: &str) -> String {
    let detail = interface_detail_text(source, span, fallback_name);
    detail
        .split_once('{')
        .map(|(signature, _)| signature.trim().to_owned())
        .unwrap_or(detail)
}

fn is_public(visibility: &AstVisibility) -> bool {
    matches!(visibility, AstVisibility::Public)
}

fn dependency_matches_import(dependency: &DependencyInterface, binding: &ImportBinding) -> bool {
    let prefix_segments = binding
        .path
        .segments
        .split_last()
        .map(|(_, prefix)| prefix)
        .unwrap_or(&[]);
    if prefix_segments.is_empty() {
        return true;
    }

    dependency
        .import_path_variants()
        .iter()
        .any(|segments| dependency_import_path_match(segments, prefix_segments).is_exact())
}

fn dependency_completion_items(
    dependency: &DependencyInterface,
    context: &DependencyImportCompletionContext,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for segments in dependency.import_path_variants() {
        match dependency_import_path_match(&segments, &context.completed_segments) {
            DependencyImportPathMatch::None => {}
            DependencyImportPathMatch::PathPrefix(next_segment) => {
                if context
                    .excluded_item_names
                    .iter()
                    .any(|name| name == next_segment)
                {
                    continue;
                }
                items.push(CompletionItem {
                    label: next_segment.to_owned(),
                    insert_text: next_segment.to_owned(),
                    kind: SymbolKind::Import,
                    detail: format!("package {}", segments.join(".")),
                    ty: None,
                });
            }
            DependencyImportPathMatch::Exact => {
                items.extend(
                    dependency
                        .symbols()
                        .iter()
                        .filter(|symbol| {
                            !context
                                .excluded_item_names
                                .iter()
                                .any(|name| name == &symbol.name)
                        })
                        .map(|symbol| CompletionItem {
                            label: symbol.name.clone(),
                            insert_text: symbol.name.clone(),
                            kind: symbol.kind,
                            detail: symbol.detail.clone(),
                            ty: None,
                        }),
                );
            }
        }
    }
    items
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DependencyImportPathMatch<'a> {
    None,
    PathPrefix(&'a str),
    Exact,
}

impl DependencyImportPathMatch<'_> {
    const fn is_exact(self) -> bool {
        matches!(self, Self::Exact)
    }
}

fn dependency_import_path_match<'a>(
    path_segments: &'a [String],
    completed_segments: &[String],
) -> DependencyImportPathMatch<'a> {
    if completed_segments.len() > path_segments.len() {
        return DependencyImportPathMatch::None;
    }

    if !completed_segments
        .iter()
        .zip(path_segments.iter())
        .all(|(left, right)| left == right)
    {
        return DependencyImportPathMatch::None;
    }

    if completed_segments.len() == path_segments.len() {
        DependencyImportPathMatch::Exact
    } else {
        DependencyImportPathMatch::PathPrefix(path_segments[completed_segments.len()].as_str())
    }
}

#[derive(Debug)]
struct DependencyImportCompletionContext {
    completed_segments: Vec<String>,
    excluded_item_names: Vec<String>,
}

fn dependency_import_completion_context(
    source: &str,
    offset: usize,
) -> Option<DependencyImportCompletionContext> {
    let offset = offset.min(source.len());
    let line_start = source[..offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|index| offset + index)
        .unwrap_or(source.len());
    let line_prefix = source.get(line_start..offset)?;
    let trimmed_prefix = line_prefix.trim_start();
    let path_prefix = trimmed_prefix.strip_prefix("use ")?;
    let line_suffix = source.get(offset..line_end)?;
    if path_prefix.contains('}') {
        return None;
    }
    if line_suffix.trim_start().starts_with("as ") {
        return None;
    }

    if path_prefix.contains('{') {
        if path_prefix.matches('{').count() != 1 {
            return None;
        }
        let (group_prefix, group_items_prefix) = path_prefix.split_once('{')?;
        let group_items = group_items_prefix.split(',').collect::<Vec<_>>();
        let current_item_prefix = group_items_prefix
            .rsplit(',')
            .next()
            .map(str::trim)
            .unwrap_or_default();
        if current_item_prefix.starts_with("as ") || current_item_prefix.contains(" as ") {
            return None;
        }
        return Some(DependencyImportCompletionContext {
            completed_segments: dependency_import_path_segments(
                group_prefix.trim().trim_end_matches('.'),
                false,
            ),
            excluded_item_names: group_items
                .iter()
                .take(group_items.len().saturating_sub(1))
                .filter_map(|item| dependency_group_item_name(item))
                .collect(),
        });
    }

    Some(DependencyImportCompletionContext {
        completed_segments: dependency_import_path_segments(
            path_prefix,
            !path_prefix.ends_with('.'),
        ),
        excluded_item_names: Vec::new(),
    })
}

fn dependency_import_path_segments(path_prefix: &str, drop_last_segment: bool) -> Vec<String> {
    let mut segments = path_prefix
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if drop_last_segment {
        segments.pop();
    }
    segments
}

fn dependency_group_item_name(item: &str) -> Option<String> {
    let item = item.trim();
    if item.is_empty() {
        return None;
    }

    item.split_once(" as ")
        .map(|(name, _)| name)
        .unwrap_or(item)
        .split('.')
        .next_back()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn dependency_variant_completion_root_offset(source: &str, offset: usize) -> Option<usize> {
    dependency_variant_reference_at(source, offset).map(|(root_offset, _, _)| root_offset)
}

fn dependency_identifier_start(source: &str, end: usize) -> usize {
    let bytes = source.as_bytes();
    let mut start = end.min(bytes.len());
    while start > 0 && is_dependency_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }
    start
}

fn is_dependency_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'`'
}

fn dependency_identifier_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut end = start.min(bytes.len());
    while end < bytes.len() && is_dependency_identifier_byte(bytes[end]) {
        end += 1;
    }
    end
}

fn dependency_variant_reference_at(source: &str, offset: usize) -> Option<(usize, Span, String)> {
    let offset = offset.min(source.len());
    let member_start = dependency_identifier_start(source, offset);
    let member_end = dependency_identifier_end(source, member_start);
    if member_start == member_end {
        return None;
    }
    if member_start == 0 || source.as_bytes().get(member_start - 1) != Some(&b'.') {
        return None;
    }

    let root_end = member_start - 1;
    let root_start = dependency_identifier_start(source, root_end);
    if root_start == root_end {
        return None;
    }
    if root_start > 0 && source.as_bytes().get(root_start - 1) == Some(&b'.') {
        return None;
    }

    let variant_name = source.get(member_start..member_end)?.to_owned();
    Some((
        root_start,
        Span::new(member_start, member_end),
        variant_name,
    ))
}

fn dependency_variant_semantic_tokens_in_module(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    source: &str,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = Vec::new();
    for (dot_offset, _) in source.match_indices('.') {
        let member_offset = dot_offset + 1;
        let Some((root_offset, span, variant_name)) =
            dependency_variant_reference_at(source, member_offset)
        else {
            continue;
        };
        if span.start != member_offset {
            continue;
        }

        let root_end = dependency_identifier_end(source, root_offset);
        let Some(root_name) = source.get(root_offset..root_end) else {
            continue;
        };
        let Some((dependency, symbol)) =
            dependency_import_binding_for_local_name(package, module, root_name)
        else {
            continue;
        };
        if dependency.variant_for(symbol, &variant_name).is_none() {
            continue;
        }

        tokens.push(SemanticTokenOccurrence {
            span,
            kind: SymbolKind::Variant,
        });
    }
    tokens
}

fn dependency_variant_detail(enum_name: &str, variant: &ql_ast::EnumVariant) -> String {
    match &variant.fields {
        ql_ast::VariantFields::Unit => format!("variant {}.{}", enum_name, variant.name),
        ql_ast::VariantFields::Tuple(items) => format!(
            "variant {}.{}({})",
            enum_name,
            variant.name,
            items
                .iter()
                .map(render_dependency_type_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ql_ast::VariantFields::Struct(fields) => format!(
            "variant {}.{} {{ {} }}",
            enum_name,
            variant.name,
            fields
                .iter()
                .map(|field| format!("{}: {}", field.name, render_dependency_type_expr(&field.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn dependency_struct_field_detail(field: &ql_ast::FieldDecl) -> String {
    format!(
        "field {}: {}",
        field.name,
        render_dependency_type_expr(&field.ty)
    )
}

fn dependency_struct_field_completion_items(
    dependency: &DependencyInterface,
    symbol: &DependencySymbol,
    excluded_field_names: &[String],
) -> Option<Vec<CompletionItem>> {
    let mut items = dependency
        .struct_decl_for(symbol)?
        .fields
        .iter()
        .filter(|field| !excluded_field_names.iter().any(|name| name == &field.name))
        .map(|field| CompletionItem {
            label: field.name.clone(),
            insert_text: field.name.clone(),
            kind: SymbolKind::Field,
            detail: dependency_struct_field_detail(field),
            ty: Some(render_dependency_type_expr(&field.ty)),
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return None;
    }
    items.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.detail.cmp(&right.detail))
    });
    Some(items)
}

const fn semantic_token_kind_sort_rank(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Function => 0,
        SymbolKind::Const => 1,
        SymbolKind::Static => 2,
        SymbolKind::Struct => 3,
        SymbolKind::Enum => 4,
        SymbolKind::Variant => 5,
        SymbolKind::Trait => 6,
        SymbolKind::TypeAlias => 7,
        SymbolKind::Field => 8,
        SymbolKind::Method => 9,
        SymbolKind::Local => 10,
        SymbolKind::Parameter => 11,
        SymbolKind::Generic => 12,
        SymbolKind::SelfParameter => 13,
        SymbolKind::BuiltinType => 14,
        SymbolKind::Import => 15,
    }
}

fn sort_and_dedup_semantic_tokens(tokens: &mut Vec<SemanticTokenOccurrence>) {
    tokens.sort_by_key(|token| {
        (
            token.span.start,
            token.span.end,
            semantic_token_kind_sort_rank(token.kind),
        )
    });
    tokens.dedup_by(|left, right| left.span == right.span && left.kind == right.kind);
}

fn collect_dependency_semantic_tokens_in_module(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    source: &str,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = dependency_variant_semantic_tokens_in_module(package, module, source);
    tokens.extend(
        package
            .dependency_value_occurrences(module)
            .into_iter()
            .map(|occurrence| SemanticTokenOccurrence {
                span: occurrence.reference_span,
                kind: occurrence.kind,
            }),
    );
    tokens.extend(
        package
            .dependency_struct_field_occurrences(module)
            .into_iter()
            .map(|occurrence| SemanticTokenOccurrence {
                span: occurrence.reference_span,
                kind: SymbolKind::Field,
            }),
    );
    tokens.extend(
        package
            .dependency_method_occurrences(module)
            .into_iter()
            .map(|occurrence| SemanticTokenOccurrence {
                span: occurrence.reference_span,
                kind: SymbolKind::Method,
            }),
    );
    tokens
}

fn collect_dependency_import_root_semantic_tokens_in_module(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    source: &str,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = Vec::new();
    for local_name in dependency_import_local_names_in_module(module) {
        let Some((_, symbol)) =
            dependency_import_binding_for_local_name(package, module, local_name.as_str())
        else {
            continue;
        };
        for (start, _) in source.match_indices(local_name.as_str()) {
            let Some(occurrence) = dependency_import_occurrence_in_module(module, start) else {
                continue;
            };
            if occurrence.local_name == local_name && occurrence.span.start == start {
                tokens.push(SemanticTokenOccurrence {
                    span: occurrence.span,
                    kind: symbol.kind,
                });
            }
        }
    }
    tokens
}

fn collect_dependency_import_root_semantic_tokens_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
) -> Vec<SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let mut unique_kinds = HashMap::new();
    let mut local_name_counts = HashMap::new();
    for binding in dependency_import_bindings_in_tokens(&tokens) {
        let Some((_, symbol)) = package.resolve_dependency_import_binding(&binding) else {
            continue;
        };
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
        unique_kinds.insert(binding.local_name, symbol.kind);
    }

    tokens
        .into_iter()
        .filter(|token| token.kind == TokenKind::Ident)
        .filter_map(|token| {
            (local_name_counts.get(token.text.as_str()) == Some(&1usize))
                .then(|| unique_kinds.get(token.text.as_str()).copied())
                .flatten()
                .map(|kind| SemanticTokenOccurrence {
                    span: token.span,
                    kind,
                })
        })
        .collect()
}

fn collect_dependency_semantic_tokens_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
) -> Vec<SemanticTokenOccurrence> {
    let mut tokens = dependency_value_occurrences_in_broken_source(package, source)
        .into_iter()
        .map(|occurrence| SemanticTokenOccurrence {
            span: occurrence.reference_span,
            kind: occurrence.kind,
        })
        .collect::<Vec<_>>();

    tokens.extend(
        dependency_member_sites_in_broken_source(source)
            .into_iter()
            .filter_map(|site| {
                let binding =
                    dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
                let kind = match site.member_kind {
                    BrokenSourceValueSegmentKind::Field => binding
                        .fields
                        .contains_key(&site.member_name)
                        .then_some(SymbolKind::Field)?,
                    BrokenSourceValueSegmentKind::Method => binding
                        .methods
                        .contains_key(&site.member_name)
                        .then_some(SymbolKind::Method)?,
                };
                Some(SemanticTokenOccurrence {
                    span: site.member_span,
                    kind,
                })
            }),
    );

    let (lexed_tokens, _) = lex(source);
    let import_targets = dependency_resolved_import_targets_in_tokens(package, &lexed_tokens);
    tokens.extend(
        lexed_tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Ident)
            .filter_map(|token| {
                let (root_offset, span, variant_name) =
                    dependency_variant_reference_at(source, token.span.start)?;
                if span != token.span {
                    return None;
                }

                let root_end = dependency_identifier_end(source, root_offset);
                let root_name = source.get(root_offset..root_end)?;
                let (_, target) = import_targets.get(root_name)?;
                if target.kind != SymbolKind::Enum {
                    return None;
                }

                let (dependency, symbol) =
                    dependency_symbol_for_broken_source_target(package, target)?;
                dependency.variant_for(symbol, &variant_name)?;
                Some(SemanticTokenOccurrence {
                    span,
                    kind: SymbolKind::Variant,
                })
            }),
    );

    tokens
}

fn dependency_import_bindings_in_tokens(tokens: &[Token]) -> Vec<ImportBinding> {
    let mut bindings = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].kind != TokenKind::Use {
            index += 1;
            continue;
        }

        let Some((next_index, use_bindings)) =
            dependency_import_bindings_after_use(tokens, index + 1)
        else {
            index += 1;
            continue;
        };
        bindings.extend(use_bindings);
        index = next_index.max(index + 1);
    }
    bindings
}

fn dependency_import_bindings_after_use(
    tokens: &[Token],
    index: usize,
) -> Option<(usize, Vec<ImportBinding>)> {
    let (prefix, mut index) = dependency_import_path_in_tokens(tokens, index)?;
    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::LBrace)
    {
        index += 2;
        let mut bindings = Vec::new();
        loop {
            if tokens.get(index).map(|token| token.kind) == Some(TokenKind::RBrace) {
                return Some((index + 1, bindings));
            }

            let item = dependency_import_ident_token(tokens, index)?;
            let item_name = item.text.clone();
            let item_span = item.span;
            index += 1;

            let (alias, alias_span, next_index) = dependency_import_alias_in_tokens(tokens, index)?;
            index = next_index;

            let mut path = prefix.clone();
            path.segments.push(item_name.clone());
            path.segment_spans.push(item_span);
            bindings.push(ImportBinding {
                local_name: alias.unwrap_or(item_name),
                definition_span: alias_span.unwrap_or(item_span),
                path,
            });

            match tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Comma) => index += 1,
                Some(TokenKind::RBrace) => return Some((index + 1, bindings)),
                _ => return None,
            }
        }
    }

    let (alias, alias_span, index) = dependency_import_alias_in_tokens(tokens, index)?;
    let local_name = alias.unwrap_or_else(|| prefix.segments.last().cloned().unwrap_or_default());
    let definition_span = alias_span
        .or_else(|| prefix.last_segment_span())
        .unwrap_or_default();
    Some((
        index,
        vec![ImportBinding {
            local_name,
            definition_span,
            path: prefix,
        }],
    ))
}

fn dependency_import_path_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(ql_ast::Path, usize)> {
    let mut index = index;
    let first = dependency_import_ident_token(tokens, index)?;
    let mut segments = vec![first.text.clone()];
    let mut spans = vec![first.span];
    index += 1;

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::Ident)
    {
        let segment = &tokens[index + 1];
        segments.push(segment.text.clone());
        spans.push(segment.span);
        index += 2;
    }

    Some((ql_ast::Path::with_spans(segments, spans), index))
}

fn dependency_import_alias_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(Option<String>, Option<Span>, usize)> {
    if tokens.get(index).map(|token| token.kind) != Some(TokenKind::As) {
        return Some((None, None, index));
    }

    let alias = dependency_import_ident_token(tokens, index + 1)?;
    Some((Some(alias.text.clone()), Some(alias.span), index + 2))
}

fn dependency_import_ident_token(tokens: &[Token], index: usize) -> Option<&Token> {
    let token = tokens.get(index)?;
    (token.kind == TokenKind::Ident).then_some(token)
}

fn dependency_import_occurrence_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
) -> Option<BrokenSourceDependencyImportOccurrence> {
    dependency_import_occurrences_in_broken_source(package, source)
        .into_iter()
        .find(|occurrence| occurrence.span.contains(offset))
}

fn dependency_import_occurrences_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
) -> Vec<BrokenSourceDependencyImportOccurrence> {
    let (tokens, _) = lex(source);
    let resolved_bindings = dependency_resolved_import_targets_in_tokens(package, &tokens);
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            let (binding, target) = resolved_bindings.get(token.text.as_str())?;
            let is_definition = token.span == binding.definition_span;
            if !is_definition
                && !dependency_import_token_matches_broken_source_reference_context(
                    &tokens,
                    index,
                    target.kind,
                )
            {
                return None;
            }
            Some(BrokenSourceDependencyImportOccurrence {
                local_name: binding.local_name.clone(),
                span: token.span,
                is_definition,
                target: target.clone(),
            })
        })
        .collect()
}

fn dependency_value_occurrence_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
) -> Option<DependencyValueOccurrence> {
    dependency_value_occurrences_in_broken_source(package, source)
        .into_iter()
        .find(|occurrence| occurrence.reference_span.contains(offset))
}

fn dependency_value_occurrences_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
) -> Vec<DependencyValueOccurrence> {
    let (tokens, _) = lex(source);
    let import_spans = dependency_import_occurrences_in_broken_source(package, source)
        .into_iter()
        .map(|occurrence| occurrence.span)
        .collect::<HashSet<_>>();
    let definition_counts = broken_source_definition_counts_in_tokens(&tokens);
    let import_targets = dependency_resolved_import_targets_in_tokens(package, &tokens);
    let import_struct_bindings = dependency_struct_import_bindings_in_tokens(package, &tokens);

    let mut bindings = broken_source_parameter_candidates_in_tokens(&tokens)
        .into_iter()
        .filter(|candidate| definition_counts.get(candidate.name.as_str()) == Some(&1usize))
        .filter_map(|candidate| {
            let type_root = candidate.type_root?;
            let dependency = import_struct_bindings.get(type_root.as_str())?.clone();
            Some(DependencyValueBinding {
                kind: SymbolKind::Parameter,
                local_name: candidate.name,
                definition_span: candidate.span,
                definition_rename: DependencyValueDefinitionRename::Direct,
                dependency,
            })
        })
        .collect::<Vec<_>>();
    let mut known_bindings = bindings
        .iter()
        .map(|binding| (binding.local_name.clone(), binding.dependency.clone()))
        .collect::<HashMap<_, _>>();

    for candidate in broken_source_local_candidates_in_tokens(&tokens) {
        if definition_counts.get(candidate.name.as_str()) != Some(&1usize) {
            continue;
        }

        let Some(rhs_value) = candidate.rhs_value.as_ref() else {
            continue;
        };
        let dependency = dependency_struct_binding_for_broken_source_value_candidate(
            package,
            &known_bindings,
            &import_struct_bindings,
            &import_targets,
            rhs_value,
        );
        let Some(dependency) = dependency else {
            continue;
        };

        known_bindings.insert(candidate.name.clone(), dependency.clone());
        bindings.push(DependencyValueBinding {
            kind: SymbolKind::Local,
            local_name: candidate.name,
            definition_span: candidate.span,
            definition_rename: DependencyValueDefinitionRename::Direct,
            dependency,
        });
    }

    let mut occurrences = Vec::new();
    for binding in bindings {
        if definition_counts.get(binding.local_name.as_str()) != Some(&1usize) {
            continue;
        }

        for (index, token) in tokens.iter().enumerate() {
            if token.kind != TokenKind::Ident || token.text != binding.local_name {
                continue;
            }
            if token.span != binding.definition_span
                && (import_spans.contains(&token.span)
                    || !dependency_value_token_matches_broken_source_reference_context(
                        &tokens, index,
                    ))
            {
                continue;
            }

            occurrences.push(DependencyValueOccurrence {
                kind: binding.kind,
                local_name: binding.local_name.clone(),
                reference_span: token.span,
                definition_span: binding.definition_span,
                definition_rename: binding.definition_rename.clone(),
                package_name: binding.dependency.package_name.clone(),
                manifest_path: binding.dependency.manifest_path.clone(),
                source_path: binding.dependency.source_path.clone(),
                struct_name: binding.dependency.struct_name.clone(),
                path: binding.dependency.path.clone(),
                is_definition: token.span == binding.definition_span,
            });
        }
    }
    occurrences.sort_by_key(|occurrence| {
        (
            occurrence.reference_span.start,
            occurrence.reference_span.end,
        )
    });
    occurrences.dedup_by(|left, right| {
        left.reference_span == right.reference_span
            && left.definition_span == right.definition_span
            && left.local_name == right.local_name
            && left.kind == right.kind
    });
    occurrences
}

fn dependency_struct_binding_for_broken_source_value_candidate(
    package: &PackageAnalysis,
    known_bindings: &HashMap<String, DependencyStructBinding>,
    import_struct_bindings: &HashMap<String, DependencyStructBinding>,
    import_targets: &HashMap<String, (ImportBinding, DependencyResolvedTarget)>,
    candidate: &BrokenSourceValueCandidate,
) -> Option<DependencyStructBinding> {
    let binding = known_bindings
        .get(candidate.root_name.as_str())
        .filter(|_| {
            !candidate.root_called
                && !candidate.root_question_unwrap
                && !candidate.root_indexed_iterable
        })
        .cloned()
        .or_else(|| {
            import_struct_bindings
                .get(candidate.root_name.as_str())
                .filter(|_| {
                    !candidate.root_called
                        && !candidate.root_question_unwrap
                        && !candidate.root_indexed_iterable
                })
                .cloned()
        })
        .or_else(|| {
            import_targets
                .get(candidate.root_name.as_str())
                .and_then(|(_, target)| {
                    dependency_struct_binding_for_broken_source_import_target(
                        package,
                        target,
                        candidate.root_called,
                        candidate.root_question_unwrap,
                        candidate.root_indexed_iterable,
                    )
                })
        })?;

    dependency_struct_binding_for_broken_source_segments(package, binding, &candidate.segments)
}

fn dependency_struct_binding_for_broken_source_segments(
    package: &PackageAnalysis,
    mut binding: DependencyStructBinding,
    segments: &[BrokenSourceValueSegment],
) -> Option<DependencyStructBinding> {
    for segment in segments {
        binding = match segment.kind {
            BrokenSourceValueSegmentKind::Field => {
                let field = binding.fields.get(&segment.name)?;
                let target = if segment.indexed_iterable {
                    if segment.question_unwrap {
                        field
                            .question_iterable_element_type_definition
                            .as_ref()
                            .or(field.iterable_element_type_definition.as_ref())?
                    } else {
                        field.iterable_element_type_definition.as_ref()?
                    }
                } else if segment.question_unwrap {
                    field
                        .question_type_definition
                        .as_ref()
                        .or(field.type_definition.as_ref())?
                } else {
                    field.type_definition.as_ref()?
                };
                dependency_struct_binding_for_definition_target(package, target)?
            }
            BrokenSourceValueSegmentKind::Method => {
                let method = binding.methods.get(&segment.name)?;
                let target = if segment.indexed_iterable {
                    if segment.question_unwrap {
                        method
                            .question_iterable_element_type_definition
                            .as_ref()
                            .or(method.iterable_element_type_definition.as_ref())?
                    } else {
                        method.iterable_element_type_definition.as_ref()?
                    }
                } else if segment.question_unwrap {
                    method
                        .question_return_type_definition
                        .as_ref()
                        .or(method.return_type_definition.as_ref())?
                } else {
                    method.return_type_definition.as_ref()?
                };
                dependency_struct_binding_for_definition_target(package, target)?
            }
        };
    }

    Some(binding)
}

fn broken_source_definition_counts_in_tokens(tokens: &[Token]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for binding in dependency_import_bindings_in_tokens(tokens) {
        *counts.entry(binding.local_name).or_insert(0) += 1;
    }
    for candidate in broken_source_parameter_candidates_in_tokens(tokens) {
        *counts.entry(candidate.name).or_insert(0) += 1;
    }
    for candidate in broken_source_local_candidates_in_tokens(tokens) {
        *counts.entry(candidate.name).or_insert(0) += 1;
    }
    for name in broken_source_item_definition_names_in_tokens(tokens) {
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

fn broken_source_parameter_candidates_in_tokens(
    tokens: &[Token],
) -> Vec<BrokenSourceParameterCandidate> {
    let mut candidates = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].kind != TokenKind::Fn {
            index += 1;
            continue;
        }

        index += 1;
        while index < tokens.len()
            && !matches!(tokens[index].kind, TokenKind::LParen | TokenKind::Eof)
        {
            index += 1;
        }
        if tokens.get(index).map(|token| token.kind) != Some(TokenKind::LParen) {
            continue;
        }

        index += 1;
        let mut depth = 1usize;
        while index < tokens.len() && depth > 0 {
            match tokens[index].kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => depth = depth.saturating_sub(1),
                TokenKind::Ident
                    if depth == 1
                        && tokens.get(index + 1).map(|token| token.kind)
                            == Some(TokenKind::Colon) =>
                {
                    candidates.push(BrokenSourceParameterCandidate {
                        name: tokens[index].text.clone(),
                        span: tokens[index].span,
                        type_root: tokens
                            .get(index + 2)
                            .filter(|token| token.kind == TokenKind::Ident)
                            .map(|token| token.text.clone()),
                    });
                }
                _ => {}
            }
            index += 1;
        }
    }
    candidates
}

fn broken_source_local_candidates_in_tokens(tokens: &[Token]) -> Vec<BrokenSourceLocalCandidate> {
    let mut candidates = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if !matches!(tokens[index].kind, TokenKind::Let | TokenKind::Var) {
            index += 1;
            continue;
        }

        let Some(name_token) = tokens
            .get(index + 1)
            .filter(|token| token.kind == TokenKind::Ident)
        else {
            index += 1;
            continue;
        };
        if !matches!(
            tokens.get(index + 2).map(|token| token.kind),
            Some(TokenKind::Eq | TokenKind::Colon)
        ) {
            index += 1;
            continue;
        }

        let mut rhs_root = None;
        let mut cursor = index + 2;
        while cursor < tokens.len() {
            match tokens[cursor].kind {
                TokenKind::Eq => {
                    rhs_root = broken_source_value_candidate_in_tokens(tokens, cursor + 1);
                    break;
                }
                TokenKind::Eof | TokenKind::RBrace => break,
                _ => cursor += 1,
            }
        }

        candidates.push(BrokenSourceLocalCandidate {
            name: name_token.text.clone(),
            span: name_token.span,
            rhs_value: rhs_root,
        });
        index += 1;
    }
    candidates
}

fn broken_source_value_candidate_in_tokens(
    tokens: &[Token],
    start_index: usize,
) -> Option<BrokenSourceValueCandidate> {
    broken_source_value_candidate_with_end_in_tokens(tokens, start_index)
        .map(|(candidate, _)| candidate)
}

fn broken_source_value_candidate_with_end_in_tokens(
    tokens: &[Token],
    start_index: usize,
) -> Option<(BrokenSourceValueCandidate, usize)> {
    let (mut candidate, mut cursor) = match tokens.get(start_index)?.kind {
        TokenKind::Ident => {
            broken_source_ident_value_candidate_with_end_in_tokens(tokens, start_index)?
        }
        TokenKind::LParen => {
            broken_source_parenthesized_value_candidate_with_end_in_tokens(tokens, start_index)?
        }
        _ => return None,
    };
    loop {
        if tokens.get(cursor).map(|token| token.kind) != Some(TokenKind::Dot) {
            break;
        }
        let name_token = tokens.get(cursor + 1)?;
        if name_token.kind != TokenKind::Ident {
            break;
        }
        cursor += 2;

        let kind = if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::LParen) {
            cursor = token_index_after_balanced_parens(tokens, cursor)?;
            BrokenSourceValueSegmentKind::Method
        } else {
            BrokenSourceValueSegmentKind::Field
        };
        let question_unwrap =
            if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::Question) {
                cursor += 1;
                true
            } else {
                false
            };
        let (next_cursor, indexed_iterable) =
            token_index_after_bracket_chain(tokens, cursor).unwrap_or((cursor, false));
        cursor = next_cursor;
        candidate.segments.push(BrokenSourceValueSegment {
            name: name_token.text.clone(),
            kind,
            question_unwrap,
            indexed_iterable,
        });
    }

    Some((candidate, cursor))
}

fn broken_source_ident_value_candidate_with_end_in_tokens(
    tokens: &[Token],
    start_index: usize,
) -> Option<(BrokenSourceValueCandidate, usize)> {
    let root = tokens.get(start_index)?;
    if root.kind != TokenKind::Ident {
        return None;
    }

    let mut cursor = start_index + 1;
    let root_called = if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::LParen) {
        cursor = token_index_after_balanced_parens(tokens, cursor)?;
        true
    } else {
        false
    };
    let root_question_unwrap =
        if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::Question) {
            cursor += 1;
            true
        } else {
            false
        };
    let (next_cursor, root_indexed_iterable) =
        token_index_after_bracket_chain(tokens, cursor).unwrap_or((cursor, false));
    cursor = next_cursor;

    Some((
        BrokenSourceValueCandidate {
            root_name: root.text.clone(),
            root_span: root.span,
            root_called,
            root_question_unwrap,
            root_indexed_iterable,
            segments: Vec::new(),
        },
        cursor,
    ))
}

fn broken_source_parenthesized_value_candidate_with_end_in_tokens(
    tokens: &[Token],
    start_index: usize,
) -> Option<(BrokenSourceValueCandidate, usize)> {
    if tokens.get(start_index).map(|token| token.kind) != Some(TokenKind::LParen) {
        return None;
    }

    let close_after = token_index_after_balanced_parens(tokens, start_index)?;
    let mut candidate =
        broken_source_parenthesized_value_candidate_in_tokens(tokens, start_index, close_after)?;
    let mut cursor = close_after;
    let outer_question_unwrap =
        if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::Question) {
            cursor += 1;
            true
        } else {
            false
        };
    if outer_question_unwrap {
        if candidate.root_question_unwrap {
            return None;
        }
        candidate.root_question_unwrap = true;
    }
    let (next_cursor, outer_indexed_iterable) =
        token_index_after_bracket_chain(tokens, cursor).unwrap_or((cursor, false));
    if outer_indexed_iterable {
        if candidate.root_indexed_iterable {
            return None;
        }
        candidate.root_indexed_iterable = true;
    }
    cursor = next_cursor;

    Some((candidate, cursor))
}

fn broken_source_parenthesized_value_candidate_in_tokens(
    tokens: &[Token],
    open_index: usize,
    close_after: usize,
) -> Option<BrokenSourceValueCandidate> {
    let inner_start = open_index + 1;
    let inner_end = close_after.checked_sub(1)?;
    if inner_start >= inner_end {
        return None;
    }

    if let Some((candidate, end)) =
        broken_source_value_candidate_with_end_in_tokens(tokens, inner_start)
        && end == inner_end
    {
        return Some(candidate);
    }

    match tokens.get(inner_start)?.kind {
        TokenKind::If => broken_source_if_value_candidate_in_tokens(tokens, inner_start, inner_end),
        TokenKind::Match => {
            broken_source_match_value_candidate_in_tokens(tokens, inner_start, inner_end)
        }
        _ => None,
    }
}

fn broken_source_if_value_candidate_in_tokens(
    tokens: &[Token],
    if_index: usize,
    end_index: usize,
) -> Option<BrokenSourceValueCandidate> {
    if tokens.get(if_index).map(|token| token.kind) != Some(TokenKind::If) {
        return None;
    }

    let then_open =
        token_index_of_top_level_kind(tokens, if_index + 1, end_index, TokenKind::LBrace)?;
    let then_close_after = token_index_after_balanced_braces(tokens, then_open)?;
    let then_candidate =
        broken_source_block_tail_value_candidate_in_tokens(tokens, then_open, then_close_after)?;
    if tokens.get(then_close_after).map(|token| token.kind) != Some(TokenKind::Else) {
        return None;
    }

    let else_start = then_close_after + 1;
    let else_candidate = match tokens.get(else_start)?.kind {
        TokenKind::LBrace => {
            let else_close_after = token_index_after_balanced_braces(tokens, else_start)?;
            if else_close_after != end_index {
                return None;
            }
            broken_source_block_tail_value_candidate_in_tokens(
                tokens,
                else_start,
                else_close_after,
            )?
        }
        TokenKind::If => broken_source_if_value_candidate_in_tokens(tokens, else_start, end_index)?,
        TokenKind::Match => {
            broken_source_match_value_candidate_in_tokens(tokens, else_start, end_index)?
        }
        _ => broken_source_value_candidate_ending_at_in_tokens(tokens, else_start, end_index)?,
    };

    broken_source_value_candidate_matches_shape(&then_candidate, &else_candidate)
        .then_some(then_candidate)
}

fn broken_source_match_value_candidate_in_tokens(
    tokens: &[Token],
    match_index: usize,
    end_index: usize,
) -> Option<BrokenSourceValueCandidate> {
    if tokens.get(match_index).map(|token| token.kind) != Some(TokenKind::Match) {
        return None;
    }

    let arms_open =
        token_index_of_top_level_kind(tokens, match_index + 1, end_index, TokenKind::LBrace)?;
    let arms_close_after = token_index_after_balanced_braces(tokens, arms_open)?;
    if arms_close_after != end_index {
        return None;
    }

    let mut cursor = arms_open + 1;
    let mut common = None::<BrokenSourceValueCandidate>;
    while cursor < end_index - 1 {
        let arrow_index =
            token_index_of_top_level_kind(tokens, cursor, end_index - 1, TokenKind::FatArrow)?;
        let body_start = arrow_index + 1;
        let candidate = match tokens.get(body_start)?.kind {
            TokenKind::LBrace => {
                let body_close_after = token_index_after_balanced_braces(tokens, body_start)?;
                let candidate = broken_source_block_tail_value_candidate_in_tokens(
                    tokens,
                    body_start,
                    body_close_after,
                )?;
                cursor = body_close_after;
                candidate
            }
            _ => {
                let body_end =
                    token_index_of_next_match_arm_separator(tokens, body_start, end_index - 1)
                        .unwrap_or(end_index - 1);
                let candidate = broken_source_value_candidate_ending_at_in_tokens(
                    tokens, body_start, body_end,
                )?;
                cursor = body_end;
                candidate
            }
        };

        match common.as_ref() {
            Some(existing)
                if !broken_source_value_candidate_matches_shape(existing, &candidate) =>
            {
                return None;
            }
            Some(_) => {}
            None => common = Some(candidate),
        }

        if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::Comma) {
            cursor += 1;
        }
    }

    common
}

fn broken_source_segments_with_stop_in_tokens(
    tokens: &[Token],
    mut cursor: usize,
    stop_index: usize,
) -> Option<Vec<BrokenSourceValueSegment>> {
    let mut segments = Vec::new();
    loop {
        if cursor == stop_index {
            return Some(segments);
        }
        if cursor > stop_index || tokens.get(cursor).map(|token| token.kind) != Some(TokenKind::Dot)
        {
            return None;
        }

        let name_token = tokens.get(cursor + 1)?;
        if name_token.kind != TokenKind::Ident {
            return None;
        }
        cursor += 2;

        let kind = if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::LParen) {
            cursor = token_index_after_balanced_parens(tokens, cursor)?;
            BrokenSourceValueSegmentKind::Method
        } else {
            BrokenSourceValueSegmentKind::Field
        };
        let question_unwrap =
            if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::Question) {
                cursor += 1;
                true
            } else {
                false
            };
        let (next_cursor, indexed_iterable) =
            token_index_after_bracket_chain(tokens, cursor).unwrap_or((cursor, false));
        cursor = next_cursor;
        if cursor > stop_index {
            return None;
        }
        segments.push(BrokenSourceValueSegment {
            name: name_token.text.clone(),
            kind,
            question_unwrap,
            indexed_iterable,
        });
    }
}

fn token_index_after_balanced_parens(tokens: &[Token], open_index: usize) -> Option<usize> {
    if tokens.get(open_index).map(|token| token.kind) != Some(TokenKind::LParen) {
        return None;
    }

    let mut depth = 1usize;
    let mut index = open_index + 1;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn token_index_after_balanced_braces(tokens: &[Token], open_index: usize) -> Option<usize> {
    if tokens.get(open_index).map(|token| token.kind) != Some(TokenKind::LBrace) {
        return None;
    }

    let mut depth = 1usize;
    let mut index = open_index + 1;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn token_index_after_balanced_brackets(tokens: &[Token], open_index: usize) -> Option<usize> {
    if tokens.get(open_index).map(|token| token.kind) != Some(TokenKind::LBracket) {
        return None;
    }

    let mut depth = 1usize;
    let mut index = open_index + 1;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LBracket => depth += 1,
            TokenKind::RBracket => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn broken_source_block_tail_value_candidate_in_tokens(
    tokens: &[Token],
    open_index: usize,
    close_after: usize,
) -> Option<BrokenSourceValueCandidate> {
    if close_after <= open_index + 1 {
        return None;
    }
    broken_source_value_candidate_ending_at_in_tokens(tokens, open_index + 1, close_after - 1)
}

fn broken_source_value_candidate_ending_at_in_tokens(
    tokens: &[Token],
    start_index: usize,
    end_index: usize,
) -> Option<BrokenSourceValueCandidate> {
    if start_index >= end_index {
        return None;
    }

    (start_index..end_index).rev().find_map(|index| {
        broken_source_value_candidate_with_end_in_tokens(tokens, index)
            .and_then(|(candidate, cursor)| (cursor == end_index).then_some(candidate))
    })
}

fn broken_source_value_candidate_with_stop_in_tokens(
    tokens: &[Token],
    start_index: usize,
    stop_index: usize,
) -> Option<BrokenSourceValueCandidate> {
    let (mut candidate, mut cursor) = match tokens.get(start_index)?.kind {
        TokenKind::Ident => {
            broken_source_ident_value_candidate_with_end_in_tokens(tokens, start_index)?
        }
        TokenKind::LParen => {
            broken_source_parenthesized_value_candidate_with_end_in_tokens(tokens, start_index)?
        }
        _ => return None,
    };
    if cursor > stop_index {
        return None;
    }
    let segments = broken_source_segments_with_stop_in_tokens(tokens, cursor, stop_index)?;
    candidate.segments.extend(segments);
    cursor = stop_index;
    (cursor == stop_index).then_some(candidate)
}

fn broken_source_value_candidate_matches_shape(
    left: &BrokenSourceValueCandidate,
    right: &BrokenSourceValueCandidate,
) -> bool {
    left.root_name == right.root_name
        && left.root_called == right.root_called
        && left.root_question_unwrap == right.root_question_unwrap
        && left.root_indexed_iterable == right.root_indexed_iterable
        && left.segments == right.segments
}

fn token_index_of_top_level_kind(
    tokens: &[Token],
    start_index: usize,
    end_index: usize,
    expected: TokenKind,
) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;
    for index in start_index..end_index {
        match tokens.get(index)?.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBrace => {
                if paren_depth == 0
                    && brace_depth == 0
                    && bracket_depth == 0
                    && expected == TokenKind::LBrace
                {
                    return Some(index);
                }
                brace_depth += 1;
            }
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            kind if kind == expected
                && paren_depth == 0
                && brace_depth == 0
                && bracket_depth == 0 =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn token_index_of_next_match_arm_separator(
    tokens: &[Token],
    start_index: usize,
    end_index: usize,
) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;
    for index in start_index..end_index {
        match tokens.get(index)?.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            TokenKind::Comma if paren_depth == 0 && brace_depth == 0 && bracket_depth == 0 => {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn token_index_after_bracket_chain(tokens: &[Token], mut cursor: usize) -> Option<(usize, bool)> {
    let mut indexed_iterable = false;
    while tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::LBracket) {
        cursor = token_index_after_balanced_brackets(tokens, cursor)?;
        indexed_iterable = true;
    }
    Some((cursor, indexed_iterable))
}

fn broken_source_item_definition_names_in_tokens(tokens: &[Token]) -> Vec<String> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| match token.kind {
            TokenKind::Fn
            | TokenKind::Const
            | TokenKind::Static
            | TokenKind::Struct
            | TokenKind::Data
            | TokenKind::Enum
            | TokenKind::Trait
            | TokenKind::Type
            | TokenKind::Opaque => tokens
                .get(index + 1)
                .filter(|next| next.kind == TokenKind::Ident)
                .map(|next| next.text.clone()),
            _ => None,
        })
        .collect()
}

fn dependency_struct_import_bindings_in_tokens(
    package: &PackageAnalysis,
    tokens: &[Token],
) -> HashMap<String, DependencyStructBinding> {
    dependency_resolved_import_targets_in_tokens(package, tokens)
        .into_iter()
        .filter_map(|(local_name, (_, target))| {
            let binding = dependency_struct_binding_for_definition_target(
                package,
                &DependencyDefinitionTarget {
                    package_name: target.package_name,
                    manifest_path: target.manifest_path,
                    source_path: target.source_path,
                    kind: target.kind,
                    name: target.name,
                    path: target.path,
                    span: target.definition_span,
                },
            )?;
            Some((local_name, binding))
        })
        .collect()
}

fn dependency_symbol_for_broken_source_target<'a>(
    package: &'a PackageAnalysis,
    target: &DependencyResolvedTarget,
) -> Option<(&'a DependencyInterface, &'a DependencySymbol)> {
    let mut exact_matches = package
        .dependencies
        .iter()
        .filter(|dependency| {
            dependency.artifact.package_name == target.package_name
                && dependency.manifest.manifest_path == target.manifest_path
        })
        .flat_map(|dependency| {
            dependency
                .symbols()
                .iter()
                .filter(move |symbol| {
                    symbol.kind == target.kind
                        && symbol.name == target.name
                        && symbol.source_path == target.source_path
                })
                .map(move |symbol| (dependency, symbol))
        })
        .collect::<Vec<_>>();
    if exact_matches.len() == 1 {
        return exact_matches.pop();
    }

    let mut loose_matches = package
        .dependencies
        .iter()
        .filter(|dependency| {
            dependency.artifact.package_name == target.package_name
                && dependency.manifest.manifest_path == target.manifest_path
        })
        .flat_map(|dependency| {
            dependency
                .symbols()
                .iter()
                .filter(move |symbol| symbol.kind == target.kind && symbol.name == target.name)
                .map(move |symbol| (dependency, symbol))
        })
        .collect::<Vec<_>>();
    if loose_matches.len() != 1 {
        return None;
    }
    loose_matches.pop()
}

fn dependency_struct_binding_for_broken_source_import_target(
    package: &PackageAnalysis,
    target: &DependencyResolvedTarget,
    root_called: bool,
    root_question_unwrap: bool,
    root_indexed_iterable: bool,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) = dependency_symbol_for_broken_source_target(package, target)?;
    let target = match target.kind {
        SymbolKind::Function if root_called => {
            if root_indexed_iterable {
                if root_question_unwrap {
                    dependency.function_question_iterable_element_type_target(symbol)?
                } else {
                    dependency.function_iterable_element_type_target(symbol)?
                }
            } else if root_question_unwrap {
                dependency.function_question_return_type_target(symbol)?
            } else {
                dependency.function_return_type_target(symbol)?
            }
        }
        SymbolKind::Const | SymbolKind::Static if !root_called => {
            if root_indexed_iterable {
                if root_question_unwrap {
                    dependency.global_question_iterable_element_type_target(symbol)?
                } else {
                    dependency.global_iterable_element_type_target(symbol)?
                }
            } else if root_question_unwrap {
                dependency.global_question_type_target(symbol)?
            } else {
                dependency.global_type_target(symbol)?
            }
        }
        _ => return None,
    };
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_member_receiver_binding_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    site: &BrokenSourceDependencyMemberSite,
) -> Option<DependencyStructBinding> {
    let candidate = &site.receiver_candidate;
    let root_binding = dependency_value_occurrences_in_broken_source(package, source)
        .into_iter()
        .find(|occurrence| {
            !candidate.root_called
                && !candidate.root_question_unwrap
                && occurrence.reference_span == candidate.root_span
        })
        .and_then(|occurrence| dependency_value_target_for_occurrence(package, &occurrence))
        .and_then(|target| {
            dependency_struct_binding_for_definition_target(
                package,
                &DependencyDefinitionTarget {
                    package_name: target.package_name,
                    manifest_path: target.manifest_path,
                    source_path: target.source_path,
                    kind: SymbolKind::Struct,
                    name: target.struct_name,
                    path: target.path,
                    span: target.definition_span,
                },
            )
        })
        .or_else(|| {
            dependency_import_occurrences_in_broken_source(package, source)
                .into_iter()
                .find(|occurrence| occurrence.span == candidate.root_span)
                .and_then(|occurrence| {
                    dependency_struct_binding_for_broken_source_import_target(
                        package,
                        &occurrence.target,
                        candidate.root_called,
                        candidate.root_question_unwrap,
                        candidate.root_indexed_iterable,
                    )
                })
        })?;
    dependency_struct_binding_for_broken_source_segments(package, root_binding, &candidate.segments)
}

fn dependency_struct_field_target_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
) -> Option<DependencyStructFieldTarget> {
    let site = dependency_member_site_in_broken_source(source, offset)?;
    if site.member_kind != BrokenSourceValueSegmentKind::Field {
        return None;
    }
    let binding = dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
    let field = binding.fields.get(&site.member_name)?.clone();
    Some(DependencyStructFieldTarget {
        reference_span: site.member_span,
        package_name: binding.package_name,
        manifest_path: binding.manifest_path,
        source_path: binding.source_path,
        struct_name: binding.struct_name,
        name: field.name.clone(),
        detail: field.detail.clone(),
        path: binding.path,
        definition_span: field.definition_span,
    })
}

fn dependency_method_target_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
) -> Option<DependencyMethodTarget> {
    let site = dependency_member_site_in_broken_source(source, offset)?;
    if site.member_kind != BrokenSourceValueSegmentKind::Method {
        return None;
    }
    let binding = dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
    let method = binding.methods.get(&site.member_name)?.clone();
    Some(DependencyMethodTarget {
        reference_span: site.member_span,
        package_name: binding.package_name,
        manifest_path: binding.manifest_path,
        source_path: method.source_path.clone(),
        struct_name: binding.struct_name,
        name: method.name.clone(),
        detail: method.detail.clone(),
        path: binding.path,
        definition_span: method.definition_span,
    })
}

fn dependency_resolved_import_targets_in_tokens(
    package: &PackageAnalysis,
    tokens: &[Token],
) -> HashMap<String, (ImportBinding, DependencyResolvedTarget)> {
    let mut grouped_bindings = HashMap::<String, Vec<ImportBinding>>::new();
    for binding in dependency_import_bindings_in_tokens(tokens) {
        grouped_bindings
            .entry(binding.local_name.clone())
            .or_default()
            .push(binding);
    }

    grouped_bindings
        .into_iter()
        .filter_map(|(local_name, bindings)| {
            let mut matches = bindings
                .into_iter()
                .filter_map(|binding| {
                    let (dependency, symbol) =
                        package.resolve_dependency_import_binding(&binding)?;
                    let definition_span = dependency.artifact_span_for(symbol)?;
                    let import_span = binding.definition_span;
                    Some((
                        binding,
                        DependencyResolvedTarget {
                            import_span,
                            package_name: dependency.artifact.package_name.clone(),
                            manifest_path: dependency.manifest.manifest_path.clone(),
                            source_path: symbol.source_path.clone(),
                            kind: symbol.kind,
                            name: symbol.name.clone(),
                            detail: symbol.detail.clone(),
                            path: dependency.interface_path.clone(),
                            definition_span,
                        },
                    ))
                })
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return None;
            }
            matches.pop().map(|resolved| (local_name, resolved))
        })
        .collect()
}

fn dependency_import_token_matches_broken_source_reference_context(
    tokens: &[Token],
    index: usize,
    kind: SymbolKind,
) -> bool {
    let prev_kind = index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.kind);
    let next_kind = tokens.get(index + 1).map(|token| token.kind);

    if matches!(prev_kind, Some(TokenKind::Dot | TokenKind::As)) {
        return false;
    }

    if matches!(
        next_kind,
        Some(
            TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::Dot
                | TokenKind::Question
        )
    ) {
        return true;
    }

    matches!(prev_kind, Some(TokenKind::Colon | TokenKind::Arrow))
        || matches!(
            (kind, next_kind),
            (
                SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait | SymbolKind::TypeAlias,
                Some(TokenKind::Comma | TokenKind::RParen | TokenKind::Eq)
            )
        )
}

fn dependency_value_token_matches_broken_source_reference_context(
    tokens: &[Token],
    index: usize,
) -> bool {
    let prev_kind = index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.kind);
    let next_kind = tokens.get(index + 1).map(|token| token.kind);

    if matches!(
        prev_kind,
        Some(
            TokenKind::Dot
                | TokenKind::As
                | TokenKind::Colon
                | TokenKind::Use
                | TokenKind::Let
                | TokenKind::Var
                | TokenKind::Fn
                | TokenKind::Const
                | TokenKind::Static
                | TokenKind::Struct
                | TokenKind::Data
                | TokenKind::Enum
                | TokenKind::Trait
                | TokenKind::Type
                | TokenKind::Opaque
                | TokenKind::Package
        )
    ) {
        return false;
    }

    !matches!(next_kind, Some(TokenKind::Colon | TokenKind::As))
}

fn dependency_import_local_names_in_module(module: &ql_ast::Module) -> HashSet<String> {
    let mut local_names = HashSet::new();
    for use_decl in &module.uses {
        if let Some(group) = &use_decl.group {
            for item in group {
                let binding = ImportBinding::grouped(&use_decl.prefix, item);
                local_names.insert(binding.local_name);
            }
            continue;
        }

        let binding = ImportBinding::direct(use_decl);
        local_names.insert(binding.local_name);
    }
    local_names
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalStructFieldCompletionSite {
    root_name: String,
    excluded_field_names: Vec<String>,
}

fn local_struct_field_completion_site(
    module: &ql_ast::Module,
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    for item in &module.items {
        if let Some(site) = local_struct_field_completion_site_in_item(item, offset) {
            return Some(site);
        }
    }
    None
}

fn local_struct_field_completion_site_in_item(
    item: &ql_ast::Item,
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    match &item.kind {
        AstItemKind::Function(function) => function
            .body
            .as_ref()
            .and_then(|body| local_struct_field_completion_site_in_block(body, offset)),
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            local_struct_field_completion_site_in_expr(&global.value, offset)
        }
        AstItemKind::Struct(struct_decl) => struct_decl.fields.iter().find_map(|field| {
            field
                .default
                .as_ref()
                .and_then(|default| local_struct_field_completion_site_in_expr(default, offset))
        }),
        AstItemKind::Trait(trait_decl) => trait_decl.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| local_struct_field_completion_site_in_block(body, offset))
        }),
        AstItemKind::Impl(impl_block) => impl_block.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| local_struct_field_completion_site_in_block(body, offset))
        }),
        AstItemKind::Extend(extend_block) => extend_block.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| local_struct_field_completion_site_in_block(body, offset))
        }),
        AstItemKind::TypeAlias(_) | AstItemKind::Enum(_) | AstItemKind::ExternBlock(_) => None,
    }
}

fn local_struct_field_completion_site_in_block(
    block: &ql_ast::Block,
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    for stmt in &block.statements {
        if let Some(site) = local_struct_field_completion_site_in_stmt(stmt, offset) {
            return Some(site);
        }
    }
    block
        .tail
        .as_ref()
        .and_then(|tail| local_struct_field_completion_site_in_expr(tail, offset))
}

fn local_struct_field_completion_site_in_stmt(
    stmt: &ql_ast::Stmt,
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    match &stmt.kind {
        ql_ast::StmtKind::Let { pattern, value, .. } => {
            local_struct_field_completion_site_in_pattern(pattern, offset)
                .or_else(|| local_struct_field_completion_site_in_expr(value, offset))
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            local_struct_field_completion_site_in_expr(expr, offset)
        }
        ql_ast::StmtKind::While { condition, body } => {
            local_struct_field_completion_site_in_expr(condition, offset)
                .or_else(|| local_struct_field_completion_site_in_block(body, offset))
        }
        ql_ast::StmtKind::Loop { body } => {
            local_struct_field_completion_site_in_block(body, offset)
        }
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => local_struct_field_completion_site_in_pattern(pattern, offset)
            .or_else(|| local_struct_field_completion_site_in_expr(iterable, offset))
            .or_else(|| local_struct_field_completion_site_in_block(body, offset)),
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn local_struct_field_completion_site_in_pattern(
    pattern: &ql_ast::Pattern,
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    match &pattern.kind {
        ql_ast::PatternKind::Tuple(items)
        | ql_ast::PatternKind::Array(items)
        | ql_ast::PatternKind::TupleStruct { items, .. } => items
            .iter()
            .find_map(|item| local_struct_field_completion_site_in_pattern(item, offset)),
        ql_ast::PatternKind::Struct { path, fields, .. } => {
            local_struct_pattern_field_completion_site(path, fields, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field.pattern.as_ref().and_then(|pattern| {
                        local_struct_field_completion_site_in_pattern(pattern, offset)
                    })
                })
            })
        }
        ql_ast::PatternKind::Name(_)
        | ql_ast::PatternKind::Path(_)
        | ql_ast::PatternKind::Integer(_)
        | ql_ast::PatternKind::String(_)
        | ql_ast::PatternKind::Bool(_)
        | ql_ast::PatternKind::NoneLiteral
        | ql_ast::PatternKind::Wildcard => None,
    }
}

fn local_struct_field_completion_site_in_expr(
    expr: &ql_ast::Expr,
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => items
            .iter()
            .find_map(|item| local_struct_field_completion_site_in_expr(item, offset)),
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            local_struct_field_completion_site_in_block(block, offset)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => local_struct_field_completion_site_in_expr(condition, offset)
            .or_else(|| local_struct_field_completion_site_in_block(then_branch, offset))
            .or_else(|| {
                else_branch
                    .as_ref()
                    .and_then(|expr| local_struct_field_completion_site_in_expr(expr, offset))
            }),
        ql_ast::ExprKind::Match { value, arms } => {
            local_struct_field_completion_site_in_expr(value, offset).or_else(|| {
                arms.iter().find_map(|arm| {
                    local_struct_field_completion_site_in_pattern(&arm.pattern, offset)
                        .or_else(|| {
                            arm.guard.as_ref().and_then(|guard| {
                                local_struct_field_completion_site_in_expr(guard, offset)
                            })
                        })
                        .or_else(|| local_struct_field_completion_site_in_expr(&arm.body, offset))
                })
            })
        }
        ql_ast::ExprKind::Closure { body, .. } => {
            local_struct_field_completion_site_in_expr(body, offset)
        }
        ql_ast::ExprKind::Call { callee, args } => {
            local_struct_field_completion_site_in_expr(callee, offset).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        local_struct_field_completion_site_in_expr(expr, offset)
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        local_struct_field_completion_site_in_expr(value, offset)
                    }
                })
            })
        }
        ql_ast::ExprKind::Member { object, .. } | ql_ast::ExprKind::Question(object) => {
            local_struct_field_completion_site_in_expr(object, offset)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            local_struct_field_completion_site_in_expr(target, offset).or_else(|| {
                items
                    .iter()
                    .find_map(|item| local_struct_field_completion_site_in_expr(item, offset))
            })
        }
        ql_ast::ExprKind::StructLiteral { path, fields } => {
            local_struct_literal_field_completion_site(path, fields, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field
                        .value
                        .as_ref()
                        .and_then(|value| local_struct_field_completion_site_in_expr(value, offset))
                })
            })
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            local_struct_field_completion_site_in_expr(left, offset)
                .or_else(|| local_struct_field_completion_site_in_expr(right, offset))
        }
        ql_ast::ExprKind::Unary { expr, .. } => {
            local_struct_field_completion_site_in_expr(expr, offset)
        }
        ql_ast::ExprKind::Name(_)
        | ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
    }
}

fn local_struct_pattern_field_completion_site(
    path: &ql_ast::Path,
    fields: &[ql_ast::PatternField],
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    let root_name = path.segments.first()?.clone();
    let current = fields.iter().find(|field| {
        field.pattern.is_some()
            && dependency_struct_field_completion_span_contains(field.name_span, offset)
    })?;
    let mut excluded_field_names = fields
        .iter()
        .filter(|field| field.name != current.name)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    excluded_field_names.sort();
    excluded_field_names.dedup();
    Some(LocalStructFieldCompletionSite {
        root_name,
        excluded_field_names,
    })
}

fn local_struct_literal_field_completion_site(
    path: &ql_ast::Path,
    fields: &[ql_ast::StructLiteralField],
    offset: usize,
) -> Option<LocalStructFieldCompletionSite> {
    let root_name = path.segments.first()?.clone();
    let current = fields.iter().find(|field| {
        field.value.is_some()
            && dependency_struct_field_completion_span_contains(field.name_span, offset)
    })?;
    let mut excluded_field_names = fields
        .iter()
        .filter(|field| field.name != current.name)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    excluded_field_names.sort();
    excluded_field_names.dedup();
    Some(LocalStructFieldCompletionSite {
        root_name,
        excluded_field_names,
    })
}

fn dependency_struct_field_completion_site(
    module: &ql_ast::Module,
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    for item in &module.items {
        if let Some(site) = dependency_struct_field_completion_site_in_item(item, offset) {
            return Some(site);
        }
    }
    None
}

fn dependency_struct_field_completion_site_in_item(
    item: &ql_ast::Item,
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    match &item.kind {
        AstItemKind::Function(function) => function
            .body
            .as_ref()
            .and_then(|body| dependency_struct_field_completion_site_in_block(body, offset)),
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            dependency_struct_field_completion_site_in_expr(&global.value, offset)
        }
        AstItemKind::Struct(struct_decl) => struct_decl.fields.iter().find_map(|field| {
            field.default.as_ref().and_then(|default| {
                dependency_struct_field_completion_site_in_expr(default, offset)
            })
        }),
        AstItemKind::Trait(trait_decl) => trait_decl.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| dependency_struct_field_completion_site_in_block(body, offset))
        }),
        AstItemKind::Impl(impl_block) => impl_block.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| dependency_struct_field_completion_site_in_block(body, offset))
        }),
        AstItemKind::Extend(extend_block) => extend_block.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| dependency_struct_field_completion_site_in_block(body, offset))
        }),
        AstItemKind::TypeAlias(_) | AstItemKind::Enum(_) | AstItemKind::ExternBlock(_) => None,
    }
}

fn dependency_struct_field_completion_site_in_block(
    block: &ql_ast::Block,
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    for stmt in &block.statements {
        if let Some(site) = dependency_struct_field_completion_site_in_stmt(stmt, offset) {
            return Some(site);
        }
    }
    block
        .tail
        .as_ref()
        .and_then(|tail| dependency_struct_field_completion_site_in_expr(tail, offset))
}

fn dependency_struct_field_completion_site_in_stmt(
    stmt: &ql_ast::Stmt,
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    match &stmt.kind {
        ql_ast::StmtKind::Let { pattern, value, .. } => {
            dependency_struct_field_completion_site_in_pattern(pattern, offset)
                .or_else(|| dependency_struct_field_completion_site_in_expr(value, offset))
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            dependency_struct_field_completion_site_in_expr(expr, offset)
        }
        ql_ast::StmtKind::While { condition, body } => {
            dependency_struct_field_completion_site_in_expr(condition, offset)
                .or_else(|| dependency_struct_field_completion_site_in_block(body, offset))
        }
        ql_ast::StmtKind::Loop { body } => {
            dependency_struct_field_completion_site_in_block(body, offset)
        }
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => dependency_struct_field_completion_site_in_pattern(pattern, offset)
            .or_else(|| dependency_struct_field_completion_site_in_expr(iterable, offset))
            .or_else(|| dependency_struct_field_completion_site_in_block(body, offset)),
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn dependency_struct_field_completion_site_in_pattern(
    pattern: &ql_ast::Pattern,
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    match &pattern.kind {
        ql_ast::PatternKind::Tuple(items)
        | ql_ast::PatternKind::Array(items)
        | ql_ast::PatternKind::TupleStruct { items, .. } => items
            .iter()
            .find_map(|item| dependency_struct_field_completion_site_in_pattern(item, offset)),
        ql_ast::PatternKind::Struct { path, fields, .. } => {
            dependency_struct_pattern_field_completion_site(path, fields, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field.pattern.as_ref().and_then(|pattern| {
                        dependency_struct_field_completion_site_in_pattern(pattern, offset)
                    })
                })
            })
        }
        ql_ast::PatternKind::Name(_)
        | ql_ast::PatternKind::Path(_)
        | ql_ast::PatternKind::Integer(_)
        | ql_ast::PatternKind::String(_)
        | ql_ast::PatternKind::Bool(_)
        | ql_ast::PatternKind::NoneLiteral
        | ql_ast::PatternKind::Wildcard => None,
    }
}

fn dependency_struct_field_completion_site_in_expr(
    expr: &ql_ast::Expr,
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => items
            .iter()
            .find_map(|item| dependency_struct_field_completion_site_in_expr(item, offset)),
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_struct_field_completion_site_in_block(block, offset)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_struct_field_completion_site_in_expr(condition, offset)
            .or_else(|| dependency_struct_field_completion_site_in_block(then_branch, offset))
            .or_else(|| {
                else_branch
                    .as_ref()
                    .and_then(|expr| dependency_struct_field_completion_site_in_expr(expr, offset))
            }),
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_struct_field_completion_site_in_expr(value, offset).or_else(|| {
                arms.iter().find_map(|arm| {
                    dependency_struct_field_completion_site_in_pattern(&arm.pattern, offset)
                        .or_else(|| {
                            arm.guard.as_ref().and_then(|guard| {
                                dependency_struct_field_completion_site_in_expr(guard, offset)
                            })
                        })
                        .or_else(|| {
                            dependency_struct_field_completion_site_in_expr(&arm.body, offset)
                        })
                })
            })
        }
        ql_ast::ExprKind::Closure { body, .. } => {
            dependency_struct_field_completion_site_in_expr(body, offset)
        }
        ql_ast::ExprKind::Call { callee, args } => {
            dependency_struct_field_completion_site_in_expr(callee, offset).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        dependency_struct_field_completion_site_in_expr(expr, offset)
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        dependency_struct_field_completion_site_in_expr(value, offset)
                    }
                })
            })
        }
        ql_ast::ExprKind::Member { object, .. } | ql_ast::ExprKind::Question(object) => {
            dependency_struct_field_completion_site_in_expr(object, offset)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            dependency_struct_field_completion_site_in_expr(target, offset).or_else(|| {
                items
                    .iter()
                    .find_map(|item| dependency_struct_field_completion_site_in_expr(item, offset))
            })
        }
        ql_ast::ExprKind::StructLiteral { path, fields } => {
            dependency_struct_literal_field_completion_site(path, fields, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field.value.as_ref().and_then(|value| {
                        dependency_struct_field_completion_site_in_expr(value, offset)
                    })
                })
            })
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_struct_field_completion_site_in_expr(left, offset)
                .or_else(|| dependency_struct_field_completion_site_in_expr(right, offset))
        }
        ql_ast::ExprKind::Unary { expr, .. } => {
            dependency_struct_field_completion_site_in_expr(expr, offset)
        }
        ql_ast::ExprKind::Name(_)
        | ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
    }
}

fn dependency_struct_pattern_field_completion_site(
    path: &ql_ast::Path,
    fields: &[ql_ast::PatternField],
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    let Some(root_name) = path.segments.first() else {
        return None;
    };
    let current = fields.iter().find(|field| {
        field.pattern.is_some()
            && dependency_struct_field_completion_span_contains(field.name_span, offset)
    })?;
    let mut excluded_field_names = fields
        .iter()
        .filter(|field| field.name != current.name)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    excluded_field_names.sort();
    excluded_field_names.dedup();
    Some(DependencyStructFieldCompletionSite {
        root_name: root_name.clone(),
        excluded_field_names,
    })
}

fn dependency_struct_literal_field_completion_site(
    path: &ql_ast::Path,
    fields: &[ql_ast::StructLiteralField],
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    let Some(root_name) = path.segments.first() else {
        return None;
    };
    let current = fields.iter().find(|field| {
        field.value.is_some()
            && dependency_struct_field_completion_span_contains(field.name_span, offset)
    })?;
    let mut excluded_field_names = fields
        .iter()
        .filter(|field| field.name != current.name)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    excluded_field_names.sort();
    excluded_field_names.dedup();
    Some(DependencyStructFieldCompletionSite {
        root_name: root_name.clone(),
        excluded_field_names,
    })
}

fn dependency_struct_field_completion_span_contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn dependency_struct_field_completion_site_in_broken_source_tokens(
    tokens: &[Token],
    offset: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    let field_index = tokens.iter().enumerate().find_map(|(index, token)| {
        (token.kind == TokenKind::Ident
            && dependency_struct_field_completion_span_contains(token.span, offset))
        .then_some(index)
    })?;
    dependency_struct_field_completion_site_from_broken_source_tokens(tokens, field_index)
}

fn dependency_struct_field_completion_site_from_broken_source_tokens(
    tokens: &[Token],
    field_index: usize,
) -> Option<DependencyStructFieldCompletionSite> {
    let field_token = tokens.get(field_index)?;
    if field_token.kind != TokenKind::Ident {
        return None;
    }

    let lbrace_index =
        dependency_struct_field_completion_lbrace_index_in_broken_source(tokens, field_index)?;
    if !dependency_struct_field_completion_is_current_field_in_broken_source(
        tokens,
        lbrace_index,
        field_index,
    ) {
        return None;
    }

    let mut excluded_field_names =
        dependency_struct_field_completion_excluded_names_in_broken_source(
            tokens,
            lbrace_index,
            field_index,
        );
    excluded_field_names.retain(|name| name != &field_token.text);
    excluded_field_names.sort();
    excluded_field_names.dedup();
    Some(DependencyStructFieldCompletionSite {
        root_name: dependency_struct_field_completion_root_name_in_broken_source(
            tokens,
            lbrace_index,
        )?,
        excluded_field_names,
    })
}

fn dependency_struct_field_completion_lbrace_index_in_broken_source(
    tokens: &[Token],
    field_index: usize,
) -> Option<usize> {
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for index in (0..field_index).rev() {
        match tokens.get(index)?.kind {
            TokenKind::RBrace => brace_depth += 1,
            TokenKind::RParen => paren_depth += 1,
            TokenKind::RBracket => bracket_depth += 1,
            TokenKind::LBrace => {
                if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 {
                    return Some(index);
                }
                brace_depth = brace_depth.saturating_sub(1);
            }
            TokenKind::LParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn dependency_struct_field_completion_root_name_in_broken_source(
    tokens: &[Token],
    lbrace_index: usize,
) -> Option<String> {
    let mut path_index = lbrace_index.checked_sub(1)?;
    while tokens.get(path_index)?.kind == TokenKind::Question {
        path_index = path_index.checked_sub(1)?;
    }
    if tokens.get(path_index)?.kind != TokenKind::Ident {
        return None;
    }

    let mut root_index = path_index;
    while root_index >= 2
        && tokens.get(root_index - 1).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(root_index - 2).map(|token| token.kind) == Some(TokenKind::Ident)
    {
        root_index -= 2;
    }
    Some(tokens.get(root_index)?.text.clone())
}

fn dependency_struct_field_completion_is_current_field_in_broken_source(
    tokens: &[Token],
    lbrace_index: usize,
    field_index: usize,
) -> bool {
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut previous_top_level = TokenKind::LBrace;
    for index in lbrace_index + 1..=field_index {
        let Some(token) = tokens.get(index) else {
            return false;
        };
        if index == field_index {
            return brace_depth == 0
                && paren_depth == 0
                && bracket_depth == 0
                && matches!(previous_top_level, TokenKind::LBrace | TokenKind::Comma);
        }

        match token.kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            _ if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                previous_top_level = token.kind;
            }
            _ => {}
        }
    }
    false
}

fn dependency_struct_field_completion_excluded_names_in_broken_source(
    tokens: &[Token],
    lbrace_index: usize,
    field_index: usize,
) -> Vec<String> {
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut previous_top_level = TokenKind::LBrace;
    let mut names = Vec::new();
    for index in lbrace_index + 1..field_index {
        let Some(token) = tokens.get(index) else {
            break;
        };
        match token.kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            _ if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                if token.kind == TokenKind::Ident
                    && matches!(previous_top_level, TokenKind::LBrace | TokenKind::Comma)
                {
                    names.push(token.text.clone());
                }
                previous_top_level = token.kind;
            }
            _ => {}
        }
    }
    names
}

fn dependency_member_site_in_broken_source(
    source: &str,
    offset: usize,
) -> Option<BrokenSourceDependencyMemberSite> {
    let (tokens, _) = lex(source);
    let member_index = tokens.iter().enumerate().find_map(|(index, token)| {
        (token.kind == TokenKind::Ident
            && dependency_struct_field_completion_span_contains(token.span, offset))
        .then_some(index)
    })?;
    dependency_member_site_in_broken_source_tokens(&tokens, member_index)
}

fn dependency_member_sites_in_broken_source(source: &str) -> Vec<BrokenSourceDependencyMemberSite> {
    let (tokens, _) = lex(source);
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            (token.kind == TokenKind::Ident)
                .then(|| dependency_member_site_in_broken_source_tokens(&tokens, index))
                .flatten()
        })
        .collect()
}

fn dependency_member_site_in_broken_source_tokens(
    tokens: &[Token],
    member_index: usize,
) -> Option<BrokenSourceDependencyMemberSite> {
    let member_token = tokens.get(member_index)?;
    if member_token.kind != TokenKind::Ident
        || tokens
            .get(member_index.checked_sub(1)?)
            .map(|token| token.kind)
            != Some(TokenKind::Dot)
    {
        return None;
    }

    let member_kind =
        if tokens.get(member_index + 1).map(|token| token.kind) == Some(TokenKind::LParen) {
            BrokenSourceValueSegmentKind::Method
        } else {
            BrokenSourceValueSegmentKind::Field
        };
    let stop_index = member_index.checked_sub(1)?;
    for start_index in (0..member_index).rev() {
        let Some(root_token) = tokens.get(start_index) else {
            continue;
        };
        if !matches!(root_token.kind, TokenKind::Ident | TokenKind::LParen) {
            continue;
        }
        if root_token.kind == TokenKind::Ident
            && start_index > 0
            && tokens.get(start_index - 1).map(|token| token.kind) == Some(TokenKind::Dot)
        {
            continue;
        }

        let Some(receiver_candidate) =
            broken_source_value_candidate_with_stop_in_tokens(tokens, start_index, stop_index)
        else {
            continue;
        };

        return Some(BrokenSourceDependencyMemberSite {
            receiver_candidate,
            member_span: member_token.span,
            member_name: member_token.text.clone(),
            member_kind,
        });
    }

    None
}

fn dependency_indexed_iterable_target_contains_block(block: &ql_ast::Block, offset: usize) -> bool {
    block
        .tail
        .as_ref()
        .is_some_and(|tail| dependency_indexed_iterable_target_contains(tail, offset))
}

fn dependency_indexed_iterable_target_contains(expr: &ql_ast::Expr, offset: usize) -> bool {
    match &expr.kind {
        ql_ast::ExprKind::Name(_) => {
            dependency_struct_field_completion_span_contains(expr.span, offset)
        }
        ql_ast::ExprKind::Member { field_span, .. } => {
            dependency_struct_field_completion_span_contains(*field_span, offset)
        }
        ql_ast::ExprKind::Call { callee, .. } => {
            dependency_indexed_iterable_target_contains(callee, offset)
        }
        ql_ast::ExprKind::Question(inner) => {
            dependency_indexed_iterable_target_contains(inner, offset)
        }
        ql_ast::ExprKind::Bracket { target, .. } => {
            dependency_indexed_iterable_target_contains(target, offset)
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_indexed_iterable_target_contains_block(block, offset)
        }
        ql_ast::ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            dependency_indexed_iterable_target_contains_block(then_branch, offset)
                || else_branch
                    .as_ref()
                    .is_some_and(|expr| dependency_indexed_iterable_target_contains(expr, offset))
        }
        ql_ast::ExprKind::Match { arms, .. } => arms
            .iter()
            .any(|arm| dependency_indexed_iterable_target_contains(&arm.body, offset)),
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DependencyMemberCompletionKind {
    Field,
    Method,
    FieldReceiver,
    MethodReceiver,
    ValueType,
}

fn dependency_member_completion_binding(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructBinding> {
    let mut scopes = vec![HashMap::new()];
    let mut iterable_scopes = vec![HashMap::new()];
    for item in &module.items {
        if let Some(binding) = dependency_member_completion_binding_in_item(
            package,
            module,
            item,
            source,
            offset,
            kind,
            &mut scopes,
            &mut iterable_scopes,
        ) {
            return Some(binding);
        }
    }
    None
}

fn dependency_member_completion_binding_in_broken_source(
    package: &PackageAnalysis,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructBinding> {
    let site = dependency_member_site_in_broken_source(source, offset)?;
    let expected_kind = match kind {
        DependencyMemberCompletionKind::Field | DependencyMemberCompletionKind::FieldReceiver => {
            BrokenSourceValueSegmentKind::Field
        }
        DependencyMemberCompletionKind::Method | DependencyMemberCompletionKind::MethodReceiver => {
            BrokenSourceValueSegmentKind::Method
        }
        DependencyMemberCompletionKind::ValueType => return None,
    };
    if site.member_kind != expected_kind {
        return None;
    }
    let member_span = site.member_span;
    let member_name = site.member_name.clone();
    let binding = dependency_member_receiver_binding_in_broken_source(package, source, &site)?;
    dependency_member_completion_binding_matches(
        &binding,
        source,
        &member_name,
        member_span,
        offset,
        kind,
    )
    .then_some(binding)
}

fn dependency_member_completion_binding_in_function(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    function: &ql_ast::FunctionDecl,
    receiver_binding: Option<&DependencyStructBinding>,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let body = function.body.as_ref()?;
    scopes.push(HashMap::new());
    iterable_scopes.push(HashMap::new());
    for param in &function.params {
        let binding = dependency_member_completion_binding_for_param(
            package,
            module,
            param,
            receiver_binding,
            offset,
            kind,
        );
        bind_dependency_struct_param(package, module, param, receiver_binding, scopes);
        bind_dependency_iterable_param(param, iterable_scopes);
        if binding.is_some() {
            iterable_scopes.pop();
            scopes.pop();
            return binding;
        }
    }
    let binding = dependency_member_completion_binding_in_block(
        package,
        module,
        body,
        source,
        offset,
        kind,
        scopes,
        iterable_scopes,
    );
    iterable_scopes.pop();
    scopes.pop();
    binding
}

fn dependency_member_completion_binding_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    match &item.kind {
        AstItemKind::Function(function) => dependency_member_completion_binding_in_function(
            package,
            module,
            function,
            None,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        ),
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            dependency_member_completion_binding_in_expr(
                package,
                module,
                &global.value,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            )
        }
        AstItemKind::Struct(struct_decl) => struct_decl.fields.iter().find_map(|field| {
            field.default.as_ref().and_then(|default| {
                dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    default,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
            })
        }),
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                let binding = dependency_member_completion_binding_in_function(
                    package,
                    module,
                    method,
                    None,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                );
                if binding.is_some() {
                    return binding;
                }
            }
            None
        }
        AstItemKind::Impl(impl_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &impl_block.target);
            for method in &impl_block.methods {
                let binding = dependency_member_completion_binding_in_function(
                    package,
                    module,
                    method,
                    receiver_binding.as_ref(),
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                );
                if binding.is_some() {
                    return binding;
                }
            }
            None
        }
        AstItemKind::Extend(extend_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &extend_block.target);
            for method in &extend_block.methods {
                let binding = dependency_member_completion_binding_in_function(
                    package,
                    module,
                    method,
                    receiver_binding.as_ref(),
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                );
                if binding.is_some() {
                    return binding;
                }
            }
            None
        }
        AstItemKind::TypeAlias(_) | AstItemKind::Enum(_) | AstItemKind::ExternBlock(_) => None,
    }
}

fn dependency_member_completion_binding_for_param(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    param: &ql_ast::Param,
    receiver_binding: Option<&DependencyStructBinding>,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructBinding> {
    if !matches!(kind, DependencyMemberCompletionKind::ValueType) {
        return None;
    }
    match param {
        ql_ast::Param::Regular { name_span, ty, .. } => {
            dependency_struct_field_completion_span_contains(*name_span, offset)
                .then(|| dependency_struct_binding_for_type_expr(package, module, ty))
                .flatten()
        }
        ql_ast::Param::Receiver { span, .. } => {
            dependency_struct_field_completion_span_contains(*span, offset)
                .then(|| receiver_binding.cloned())
                .flatten()
        }
    }
}

fn dependency_member_completion_binding_for_closure_param(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    param: &ql_ast::ClosureParam,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructBinding> {
    if !matches!(kind, DependencyMemberCompletionKind::ValueType) {
        return None;
    }
    let ty = param.ty.as_ref()?;
    dependency_struct_field_completion_span_contains(param.span, offset)
        .then(|| dependency_struct_binding_for_type_expr(package, module, ty))
        .flatten()
}

fn dependency_member_completion_binding_for_pattern(
    package: &PackageAnalysis,
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> Option<DependencyStructBinding> {
    if !matches!(kind, DependencyMemberCompletionKind::ValueType) {
        return None;
    }
    match &pattern.kind {
        ql_ast::PatternKind::Name(_) => {
            dependency_struct_field_completion_span_contains(pattern.span, offset)
                .then_some(binding.clone())
        }
        ql_ast::PatternKind::Tuple(items) | ql_ast::PatternKind::Array(items) => {
            items.iter().find_map(|item| {
                dependency_member_completion_binding_for_pattern(
                    package, item, binding, offset, kind,
                )
            })
        }
        ql_ast::PatternKind::Struct { fields, .. } => fields.iter().find_map(|field| {
            let field_binding = binding
                .fields
                .get(&field.name)
                .and_then(|field| dependency_struct_binding_for_resolved_field(package, field))?;
            field.pattern.as_ref().and_then(|pattern| {
                dependency_member_completion_binding_for_pattern(
                    package,
                    pattern,
                    &field_binding,
                    offset,
                    kind,
                )
            })
        }),
        _ => None,
    }
}

fn dependency_member_completion_binding_in_block(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    scopes.push(HashMap::new());
    iterable_scopes.push(HashMap::new());
    for stmt in &block.statements {
        if let Some(binding) = dependency_member_completion_binding_in_stmt(
            package,
            module,
            stmt,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        ) {
            iterable_scopes.pop();
            scopes.pop();
            return Some(binding);
        }
    }
    let binding = block.tail.as_ref().and_then(|tail| {
        dependency_member_completion_binding_in_expr(
            package,
            module,
            tail,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        )
    });
    iterable_scopes.pop();
    scopes.pop();
    binding
}

fn dependency_member_completion_binding_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    match &stmt.kind {
        ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } => {
            let expr_binding = dependency_member_completion_binding_in_expr(
                package,
                module,
                value,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            );
            let let_binding = ty
                .as_ref()
                .and_then(|ty| dependency_struct_binding_for_type_expr(package, module, ty))
                .or_else(|| dependency_struct_binding_for_expr(package, module, value, scopes));
            let pattern_binding = let_binding.as_ref().and_then(|binding| {
                dependency_member_completion_binding_for_pattern(
                    package, pattern, binding, offset, kind,
                )
            });
            bind_dependency_struct_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                scopes,
                iterable_scopes,
            );
            bind_dependency_iterable_let(package, module, pattern, value, scopes, iterable_scopes);
            expr_binding.or(pattern_binding)
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => dependency_member_completion_binding_in_expr(
            package,
            module,
            expr,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        ),
        ql_ast::StmtKind::While { condition, body } => {
            dependency_member_completion_binding_in_expr(
                package,
                module,
                condition,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            )
            .or_else(|| {
                dependency_member_completion_binding_in_block(
                    package,
                    module,
                    body,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
            })
        }
        ql_ast::StmtKind::Loop { body } => dependency_member_completion_binding_in_block(
            package,
            module,
            body,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        ),
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            let iterable_binding = dependency_struct_element_binding_for_iterable_expr(
                package,
                module,
                iterable,
                scopes,
                iterable_scopes,
            );
            let expr_binding = dependency_member_completion_binding_in_expr(
                package,
                module,
                iterable,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            );
            let pattern_binding = iterable_binding.as_ref().and_then(|binding| {
                dependency_member_completion_binding_for_pattern(
                    package, pattern, binding, offset, kind,
                )
            });

            scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            if let Some(binding) = &iterable_binding {
                bind_dependency_struct_pattern(package, pattern, binding, scopes);
            }
            shadow_dependency_iterable_pattern(pattern, iterable_scopes);
            let body_binding = dependency_member_completion_binding_in_block(
                package,
                module,
                body,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            );
            iterable_scopes.pop();
            scopes.pop();

            expr_binding.or(pattern_binding).or(body_binding)
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn dependency_member_completion_binding_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    source: &str,
    offset: usize,
    kind: DependencyMemberCompletionKind,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            items.iter().find_map(|item| {
                dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    item,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
            })
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_member_completion_binding_in_block(
                package,
                module,
                block,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            )
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_member_completion_binding_in_expr(
            package,
            module,
            condition,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        )
        .or_else(|| {
            dependency_member_completion_binding_in_block(
                package,
                module,
                then_branch,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            )
        })
        .or_else(|| {
            else_branch.as_ref().and_then(|expr| {
                dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    expr,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
            })
        }),
        ql_ast::ExprKind::Match { value, arms } => dependency_member_completion_binding_in_expr(
            package,
            module,
            value,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        )
        .or_else(|| {
            let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
                iterable_scopes.push(HashMap::new());
                let pattern_binding = value_binding.as_ref().and_then(|binding| {
                    dependency_member_completion_binding_for_pattern(
                        package,
                        &arm.pattern,
                        binding,
                        offset,
                        kind,
                    )
                });
                if let Some(binding) = &value_binding {
                    bind_dependency_struct_match_pattern(package, &arm.pattern, binding, scopes);
                }
                shadow_dependency_iterable_pattern(&arm.pattern, iterable_scopes);
                let binding = pattern_binding.or_else(|| {
                    arm.guard
                        .as_ref()
                        .and_then(|guard| {
                            dependency_member_completion_binding_in_expr(
                                package,
                                module,
                                guard,
                                source,
                                offset,
                                kind,
                                scopes,
                                iterable_scopes,
                            )
                        })
                        .or_else(|| {
                            dependency_member_completion_binding_in_expr(
                                package,
                                module,
                                &arm.body,
                                source,
                                offset,
                                kind,
                                scopes,
                                iterable_scopes,
                            )
                        })
                });
                iterable_scopes.pop();
                scopes.pop();
                if binding.is_some() {
                    return binding;
                }
            }
            None
        }),
        ql_ast::ExprKind::Closure { params, body, .. } => {
            scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            for param in params {
                let binding = dependency_member_completion_binding_for_closure_param(
                    package, module, param, offset, kind,
                );
                bind_dependency_struct_closure_param(package, module, param, scopes);
                bind_dependency_iterable_closure_param(param, iterable_scopes);
                if binding.is_some() {
                    iterable_scopes.pop();
                    scopes.pop();
                    return binding;
                }
            }
            let binding = dependency_member_completion_binding_in_expr(
                package,
                module,
                body,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            );
            iterable_scopes.pop();
            scopes.pop();
            binding
        }
        ql_ast::ExprKind::Call { callee, args } => dependency_member_completion_binding_in_expr(
            package,
            module,
            callee,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        )
        .or_else(|| {
            args.iter().find_map(|arg| match arg {
                ql_ast::CallArg::Positional(expr) => dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    expr,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                ),
                ql_ast::CallArg::Named { value, .. } => {
                    dependency_member_completion_binding_in_expr(
                        package,
                        module,
                        value,
                        source,
                        offset,
                        kind,
                        scopes,
                        iterable_scopes,
                    )
                }
            })
        }),
        ql_ast::ExprKind::Name(name) => {
            if !matches!(kind, DependencyMemberCompletionKind::ValueType)
                || !dependency_struct_field_completion_span_contains(expr.span, offset)
            {
                return None;
            }
            dependency_struct_binding_for_name(scopes, name)
        }
        ql_ast::ExprKind::Member {
            object,
            field,
            field_span,
        } => dependency_member_completion_binding_in_expr(
            package,
            module,
            object,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        )
        .or_else(|| {
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            dependency_member_completion_binding_matches(
                &binding,
                source,
                field,
                *field_span,
                offset,
                kind,
            )
            .then_some(binding)
        }),
        ql_ast::ExprKind::Bracket { target, items } => {
            if matches!(kind, DependencyMemberCompletionKind::ValueType)
                && dependency_indexed_iterable_target_contains(target, offset)
            {
                dependency_struct_element_binding_for_iterable_expr(
                    package,
                    module,
                    target,
                    scopes,
                    iterable_scopes,
                )
                .or_else(|| {
                    dependency_member_completion_binding_in_expr(
                        package,
                        module,
                        target,
                        source,
                        offset,
                        kind,
                        scopes,
                        iterable_scopes,
                    )
                })
                .or_else(|| {
                    items.iter().find_map(|item| {
                        dependency_member_completion_binding_in_expr(
                            package,
                            module,
                            item,
                            source,
                            offset,
                            kind,
                            scopes,
                            iterable_scopes,
                        )
                    })
                })
            } else {
                dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    target,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
                .or_else(|| {
                    items.iter().find_map(|item| {
                        dependency_member_completion_binding_in_expr(
                            package,
                            module,
                            item,
                            source,
                            offset,
                            kind,
                            scopes,
                            iterable_scopes,
                        )
                    })
                })
            }
        }
        ql_ast::ExprKind::StructLiteral { fields, .. } => fields.iter().find_map(|field| {
            field.value.as_ref().and_then(|value| {
                dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    value,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
            })
        }),
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_member_completion_binding_in_expr(
                package,
                module,
                left,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            )
            .or_else(|| {
                dependency_member_completion_binding_in_expr(
                    package,
                    module,
                    right,
                    source,
                    offset,
                    kind,
                    scopes,
                    iterable_scopes,
                )
            })
        }
        ql_ast::ExprKind::Unary { expr, .. } => dependency_member_completion_binding_in_expr(
            package,
            module,
            expr,
            source,
            offset,
            kind,
            scopes,
            iterable_scopes,
        ),
        ql_ast::ExprKind::Question(expr) => {
            if matches!(kind, DependencyMemberCompletionKind::ValueType) {
                match &expr.kind {
                    ql_ast::ExprKind::Member { field_span, .. } if field_span.contains(offset) => {
                        return dependency_struct_binding_for_question_expr(
                            package, module, expr, scopes,
                        );
                    }
                    ql_ast::ExprKind::Call { callee, .. }
                        if matches!(
                            &callee.kind,
                            ql_ast::ExprKind::Member { field_span, .. } if field_span.contains(offset)
                        ) =>
                    {
                        return dependency_struct_binding_for_question_expr(
                            package, module, expr, scopes,
                        );
                    }
                    _ => {}
                }
            }
            dependency_member_completion_binding_in_expr(
                package,
                module,
                expr,
                source,
                offset,
                kind,
                scopes,
                iterable_scopes,
            )
        }
        ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
    }
}

fn dependency_member_completion_binding_matches(
    binding: &DependencyStructBinding,
    source: &str,
    field_name: &str,
    field_span: Span,
    offset: usize,
    kind: DependencyMemberCompletionKind,
) -> bool {
    if !dependency_struct_field_completion_span_contains(field_span, offset) {
        return false;
    }
    let field_prefix_match = binding
        .fields
        .keys()
        .any(|name| name.starts_with(field_name));
    let method_prefix_match = binding
        .methods
        .keys()
        .any(|name| name.starts_with(field_name));
    let next_non_whitespace = source
        .get(field_span.end..)
        .and_then(|suffix| suffix.chars().find(|ch| !ch.is_whitespace()));
    match kind {
        DependencyMemberCompletionKind::Field => {
            field_prefix_match && next_non_whitespace != Some('(')
        }
        DependencyMemberCompletionKind::Method => {
            method_prefix_match && (next_non_whitespace == Some('(') || !field_prefix_match)
        }
        DependencyMemberCompletionKind::FieldReceiver => next_non_whitespace != Some('('),
        DependencyMemberCompletionKind::MethodReceiver => next_non_whitespace == Some('('),
        DependencyMemberCompletionKind::ValueType => false,
    }
}

fn dependency_question_wrapped_field_reference_in_module(
    module: &ql_ast::Module,
    offset: usize,
) -> bool {
    module.items.iter().any(|item| {
        dependency_question_wrapped_reference_in_item(
            item,
            offset,
            DependencyQuestionWrappedReferenceKind::Field,
        )
    })
}

fn dependency_question_wrapped_method_reference_in_module(
    module: &ql_ast::Module,
    offset: usize,
) -> bool {
    module.items.iter().any(|item| {
        dependency_question_wrapped_reference_in_item(
            item,
            offset,
            DependencyQuestionWrappedReferenceKind::Method,
        )
    })
}

fn dependency_question_wrapped_field_reference_in_broken_source(
    source: &str,
    offset: usize,
) -> bool {
    let site = match dependency_member_site_in_broken_source(source, offset) {
        Some(site) if site.member_kind == BrokenSourceValueSegmentKind::Field => site,
        _ => return false,
    };
    dependency_immediate_member_receiver_is_question_unwrapped(&site)
        || dependency_broken_source_field_reference_has_trailing_question(source, offset)
}

fn dependency_question_wrapped_method_reference_in_broken_source(
    source: &str,
    offset: usize,
) -> bool {
    let (tokens, member_index) =
        match dependency_member_tokens_and_index_in_broken_source(source, offset) {
            Some(value) => value,
            None => return false,
        };
    if tokens.get(member_index + 1).map(|token| token.kind) != Some(TokenKind::LParen) {
        return false;
    }
    token_index_after_balanced_parens(&tokens, member_index + 1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.kind)
        == Some(TokenKind::Question)
}

fn dependency_immediate_member_receiver_is_question_unwrapped(
    site: &BrokenSourceDependencyMemberSite,
) -> bool {
    site.receiver_candidate
        .segments
        .last()
        .map(|segment| segment.question_unwrap)
        .unwrap_or(site.receiver_candidate.root_question_unwrap)
}

fn dependency_broken_source_field_reference_has_trailing_question(
    source: &str,
    offset: usize,
) -> bool {
    let (tokens, member_index) =
        match dependency_member_tokens_and_index_in_broken_source(source, offset) {
            Some(value) => value,
            None => return false,
        };
    tokens.get(member_index + 1).map(|token| token.kind) == Some(TokenKind::Question)
}

fn dependency_member_tokens_and_index_in_broken_source(
    source: &str,
    offset: usize,
) -> Option<(Vec<Token>, usize)> {
    let (tokens, _) = lex(source);
    let member_index = tokens.iter().enumerate().find_map(|(index, token)| {
        (token.kind == TokenKind::Ident
            && dependency_struct_field_completion_span_contains(token.span, offset))
        .then_some(index)
    })?;
    Some((tokens, member_index))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DependencyQuestionWrappedReferenceKind {
    Field,
    Method,
}

fn dependency_question_wrapped_reference_in_item(
    item: &ql_ast::Item,
    offset: usize,
    kind: DependencyQuestionWrappedReferenceKind,
) -> bool {
    match &item.kind {
        ql_ast::ItemKind::Function(function) => function
            .body
            .as_ref()
            .is_some_and(|body| dependency_question_wrapped_reference_in_block(body, offset, kind)),
        ql_ast::ItemKind::Const(global) | ql_ast::ItemKind::Static(global) => {
            dependency_question_wrapped_reference_in_expr(&global.value, offset, kind)
        }
        ql_ast::ItemKind::Impl(block) => block.methods.iter().any(|method| {
            method.body.as_ref().is_some_and(|body| {
                dependency_question_wrapped_reference_in_block(body, offset, kind)
            })
        }),
        ql_ast::ItemKind::Extend(block) => block.methods.iter().any(|method| {
            method.body.as_ref().is_some_and(|body| {
                dependency_question_wrapped_reference_in_block(body, offset, kind)
            })
        }),
        _ => false,
    }
}

fn dependency_question_wrapped_reference_in_block(
    block: &ql_ast::Block,
    offset: usize,
    kind: DependencyQuestionWrappedReferenceKind,
) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| dependency_question_wrapped_reference_in_stmt(stmt, offset, kind))
        || block
            .tail
            .as_ref()
            .is_some_and(|expr| dependency_question_wrapped_reference_in_expr(expr, offset, kind))
}

fn dependency_question_wrapped_reference_in_stmt(
    stmt: &ql_ast::Stmt,
    offset: usize,
    kind: DependencyQuestionWrappedReferenceKind,
) -> bool {
    match &stmt.kind {
        ql_ast::StmtKind::Let { value, .. }
        | ql_ast::StmtKind::Return(Some(value))
        | ql_ast::StmtKind::Defer(value)
        | ql_ast::StmtKind::Expr { expr: value, .. } => {
            dependency_question_wrapped_reference_in_expr(value, offset, kind)
        }
        ql_ast::StmtKind::While { condition, body } => {
            dependency_question_wrapped_reference_in_expr(condition, offset, kind)
                || dependency_question_wrapped_reference_in_block(body, offset, kind)
        }
        ql_ast::StmtKind::Loop { body } => {
            dependency_question_wrapped_reference_in_block(body, offset, kind)
        }
        ql_ast::StmtKind::For { iterable, body, .. } => {
            dependency_question_wrapped_reference_in_expr(iterable, offset, kind)
                || dependency_question_wrapped_reference_in_block(body, offset, kind)
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            false
        }
    }
}

fn dependency_question_wrapped_reference_in_expr(
    expr: &ql_ast::Expr,
    offset: usize,
    kind: DependencyQuestionWrappedReferenceKind,
) -> bool {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => items
            .iter()
            .any(|item| dependency_question_wrapped_reference_in_expr(item, offset, kind)),
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_question_wrapped_reference_in_block(block, offset, kind)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            dependency_question_wrapped_reference_in_expr(condition, offset, kind)
                || dependency_question_wrapped_reference_in_block(then_branch, offset, kind)
                || else_branch.as_ref().is_some_and(|expr| {
                    dependency_question_wrapped_reference_in_expr(expr, offset, kind)
                })
        }
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_question_wrapped_reference_in_expr(value, offset, kind)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| {
                        dependency_question_wrapped_reference_in_expr(guard, offset, kind)
                    }) || dependency_question_wrapped_reference_in_expr(&arm.body, offset, kind)
                })
        }
        ql_ast::ExprKind::Closure { body, .. } => {
            dependency_question_wrapped_reference_in_expr(body, offset, kind)
        }
        ql_ast::ExprKind::Call { callee, args } => {
            dependency_question_wrapped_reference_in_expr(callee, offset, kind)
                || args.iter().any(|arg| match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        dependency_question_wrapped_reference_in_expr(expr, offset, kind)
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        dependency_question_wrapped_reference_in_expr(value, offset, kind)
                    }
                })
        }
        ql_ast::ExprKind::Member { object, .. } => {
            dependency_question_wrapped_reference_in_expr(object, offset, kind)
        }
        ql_ast::ExprKind::Question(inner) => {
            dependency_question_wrapped_reference_matches(inner, offset, kind)
                || dependency_question_wrapped_reference_in_expr(inner, offset, kind)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            dependency_question_wrapped_reference_in_expr(target, offset, kind)
                || items
                    .iter()
                    .any(|item| dependency_question_wrapped_reference_in_expr(item, offset, kind))
        }
        ql_ast::ExprKind::StructLiteral { fields, .. } => fields.iter().any(|field| {
            field.value.as_ref().is_some_and(|value| {
                dependency_question_wrapped_reference_in_expr(value, offset, kind)
            })
        }),
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_question_wrapped_reference_in_expr(left, offset, kind)
                || dependency_question_wrapped_reference_in_expr(right, offset, kind)
        }
        ql_ast::ExprKind::Unary { expr, .. } => {
            dependency_question_wrapped_reference_in_expr(expr, offset, kind)
        }
        ql_ast::ExprKind::Name(_)
        | ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => false,
    }
}

fn dependency_question_wrapped_reference_matches(
    expr: &ql_ast::Expr,
    offset: usize,
    kind: DependencyQuestionWrappedReferenceKind,
) -> bool {
    match kind {
        DependencyQuestionWrappedReferenceKind::Field => {
            matches!(&expr.kind, ql_ast::ExprKind::Member { field_span, .. } if field_span.contains(offset))
        }
        DependencyQuestionWrappedReferenceKind::Method => matches!(
            &expr.kind,
            ql_ast::ExprKind::Call { callee, .. }
                if matches!(
                    &callee.kind,
                    ql_ast::ExprKind::Member { field_span, .. } if field_span.contains(offset)
                )
        ),
    }
}

fn dependency_import_occurrence_in_module(
    module: &ql_ast::Module,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    for use_decl in &module.uses {
        if let Some(occurrence) = dependency_import_occurrence_in_use_decl(use_decl, offset) {
            return Some(occurrence);
        }
    }
    for item in &module.items {
        if let Some(occurrence) = dependency_import_occurrence_in_item(item, offset) {
            return Some(occurrence);
        }
    }
    None
}

fn dependency_type_import_occurrence_in_module(
    module: &ql_ast::Module,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    module
        .items
        .iter()
        .find_map(|item| dependency_type_import_occurrence_in_item(item, offset))
}

fn dependency_import_occurrence_in_use_decl(
    use_decl: &ql_ast::UseDecl,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    if let Some(group) = &use_decl.group {
        for item in group {
            let binding = ImportBinding::grouped(&use_decl.prefix, item);
            if dependency_struct_field_completion_span_contains(binding.definition_span, offset) {
                return Some(DependencyImportOccurrence {
                    local_name: binding.local_name,
                    span: binding.definition_span,
                    is_definition: true,
                });
            }
        }
        return None;
    }

    let binding = ImportBinding::direct(use_decl);
    dependency_struct_field_completion_span_contains(binding.definition_span, offset).then_some(
        DependencyImportOccurrence {
            local_name: binding.local_name,
            span: binding.definition_span,
            is_definition: true,
        },
    )
}

fn dependency_import_occurrence_in_item(
    item: &ql_ast::Item,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &item.kind {
        AstItemKind::Function(function) => {
            dependency_import_occurrence_in_function(function, offset)
        }
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            dependency_import_occurrence_in_type_expr(&global.ty, offset)
                .or_else(|| dependency_import_occurrence_in_expr(&global.value, offset))
        }
        AstItemKind::Struct(struct_decl) => struct_decl.fields.iter().find_map(|field| {
            dependency_import_occurrence_in_type_expr(&field.ty, offset).or_else(|| {
                field
                    .default
                    .as_ref()
                    .and_then(|default| dependency_import_occurrence_in_expr(default, offset))
            })
        }),
        AstItemKind::Enum(enum_decl) => {
            enum_decl
                .variants
                .iter()
                .find_map(|variant| match &variant.fields {
                    ql_ast::VariantFields::Unit => None,
                    ql_ast::VariantFields::Tuple(items) => items
                        .iter()
                        .find_map(|item| dependency_import_occurrence_in_type_expr(item, offset)),
                    ql_ast::VariantFields::Struct(fields) => fields.iter().find_map(|field| {
                        dependency_import_occurrence_in_type_expr(&field.ty, offset).or_else(|| {
                            field.default.as_ref().and_then(|default| {
                                dependency_import_occurrence_in_expr(default, offset)
                            })
                        })
                    }),
                })
        }
        AstItemKind::Trait(trait_decl) => trait_decl
            .methods
            .iter()
            .find_map(|method| dependency_import_occurrence_in_function(method, offset)),
        AstItemKind::Impl(impl_block) => {
            dependency_import_occurrence_in_type_expr(&impl_block.target, offset)
                .or_else(|| {
                    impl_block
                        .trait_ty
                        .as_ref()
                        .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
                })
                .or_else(|| {
                    impl_block.where_clause.iter().find_map(|predicate| {
                        dependency_import_occurrence_in_where_predicate(predicate, offset)
                    })
                })
                .or_else(|| {
                    impl_block
                        .methods
                        .iter()
                        .find_map(|method| dependency_import_occurrence_in_function(method, offset))
                })
        }
        AstItemKind::Extend(extend_block) => {
            dependency_import_occurrence_in_type_expr(&extend_block.target, offset).or_else(|| {
                extend_block
                    .methods
                    .iter()
                    .find_map(|method| dependency_import_occurrence_in_function(method, offset))
            })
        }
        AstItemKind::TypeAlias(type_alias) => {
            dependency_import_occurrence_in_type_expr(&type_alias.ty, offset)
        }
        AstItemKind::ExternBlock(extern_block) => extern_block
            .functions
            .iter()
            .find_map(|function| dependency_import_occurrence_in_function(function, offset)),
    }
}

fn dependency_type_import_occurrence_in_item(
    item: &ql_ast::Item,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &item.kind {
        AstItemKind::Function(function) => {
            dependency_type_import_occurrence_in_function(function, offset)
        }
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            dependency_import_occurrence_in_type_expr(&global.ty, offset)
                .or_else(|| dependency_type_import_occurrence_in_expr(&global.value, offset))
        }
        AstItemKind::Struct(struct_decl) => struct_decl.fields.iter().find_map(|field| {
            dependency_import_occurrence_in_type_expr(&field.ty, offset).or_else(|| {
                field
                    .default
                    .as_ref()
                    .and_then(|default| dependency_type_import_occurrence_in_expr(default, offset))
            })
        }),
        AstItemKind::Enum(enum_decl) => {
            enum_decl
                .variants
                .iter()
                .find_map(|variant| match &variant.fields {
                    ql_ast::VariantFields::Unit => None,
                    ql_ast::VariantFields::Tuple(items) => items
                        .iter()
                        .find_map(|item| dependency_import_occurrence_in_type_expr(item, offset)),
                    ql_ast::VariantFields::Struct(fields) => fields.iter().find_map(|field| {
                        dependency_import_occurrence_in_type_expr(&field.ty, offset).or_else(|| {
                            field.default.as_ref().and_then(|default| {
                                dependency_type_import_occurrence_in_expr(default, offset)
                            })
                        })
                    }),
                })
        }
        AstItemKind::Trait(trait_decl) => trait_decl
            .methods
            .iter()
            .find_map(|method| dependency_type_import_occurrence_in_function(method, offset)),
        AstItemKind::Impl(impl_block) => {
            dependency_import_occurrence_in_type_expr(&impl_block.target, offset)
                .or_else(|| {
                    impl_block
                        .trait_ty
                        .as_ref()
                        .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
                })
                .or_else(|| {
                    impl_block.where_clause.iter().find_map(|predicate| {
                        dependency_type_import_occurrence_in_where_predicate(predicate, offset)
                    })
                })
                .or_else(|| {
                    impl_block.methods.iter().find_map(|method| {
                        dependency_type_import_occurrence_in_function(method, offset)
                    })
                })
        }
        AstItemKind::Extend(extend_block) => {
            dependency_import_occurrence_in_type_expr(&extend_block.target, offset).or_else(|| {
                extend_block.methods.iter().find_map(|method| {
                    dependency_type_import_occurrence_in_function(method, offset)
                })
            })
        }
        AstItemKind::TypeAlias(type_alias) => {
            dependency_import_occurrence_in_type_expr(&type_alias.ty, offset)
        }
        AstItemKind::ExternBlock(extern_block) => extern_block
            .functions
            .iter()
            .find_map(|function| dependency_type_import_occurrence_in_function(function, offset)),
    }
}

fn dependency_import_occurrence_in_function(
    function: &ql_ast::FunctionDecl,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    function
        .params
        .iter()
        .find_map(|param| match param {
            ql_ast::Param::Regular { ty, .. } => {
                dependency_import_occurrence_in_type_expr(ty, offset)
            }
            ql_ast::Param::Receiver { .. } => None,
        })
        .or_else(|| {
            function
                .return_type
                .as_ref()
                .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
        })
        .or_else(|| {
            function.where_clause.iter().find_map(|predicate| {
                dependency_import_occurrence_in_where_predicate(predicate, offset)
            })
        })
        .or_else(|| {
            function
                .body
                .as_ref()
                .and_then(|body| dependency_import_occurrence_in_block(body, offset))
        })
}

fn dependency_type_import_occurrence_in_function(
    function: &ql_ast::FunctionDecl,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    function
        .params
        .iter()
        .find_map(|param| match param {
            ql_ast::Param::Regular { ty, .. } => {
                dependency_import_occurrence_in_type_expr(ty, offset)
            }
            ql_ast::Param::Receiver { .. } => None,
        })
        .or_else(|| {
            function
                .return_type
                .as_ref()
                .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
        })
        .or_else(|| {
            function.where_clause.iter().find_map(|predicate| {
                dependency_type_import_occurrence_in_where_predicate(predicate, offset)
            })
        })
        .or_else(|| {
            function
                .body
                .as_ref()
                .and_then(|body| dependency_type_import_occurrence_in_block(body, offset))
        })
}

fn dependency_import_occurrence_in_where_predicate(
    predicate: &ql_ast::WherePredicate,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    dependency_import_occurrence_in_type_expr(&predicate.target, offset).or_else(|| {
        predicate
            .bounds
            .iter()
            .find_map(|bound| dependency_import_occurrence_for_path(bound, offset))
    })
}

fn dependency_type_import_occurrence_in_where_predicate(
    predicate: &ql_ast::WherePredicate,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    dependency_import_occurrence_in_type_expr(&predicate.target, offset).or_else(|| {
        predicate
            .bounds
            .iter()
            .find_map(|bound| dependency_import_occurrence_for_path(bound, offset))
    })
}

fn dependency_import_occurrence_in_block(
    block: &ql_ast::Block,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    for stmt in &block.statements {
        if let Some(occurrence) = dependency_import_occurrence_in_stmt(stmt, offset) {
            return Some(occurrence);
        }
    }
    block
        .tail
        .as_ref()
        .and_then(|tail| dependency_import_occurrence_in_expr(tail, offset))
}

fn dependency_type_import_occurrence_in_block(
    block: &ql_ast::Block,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    for stmt in &block.statements {
        if let Some(occurrence) = dependency_type_import_occurrence_in_stmt(stmt, offset) {
            return Some(occurrence);
        }
    }
    block
        .tail
        .as_ref()
        .and_then(|tail| dependency_type_import_occurrence_in_expr(tail, offset))
}

fn dependency_import_occurrence_in_stmt(
    stmt: &ql_ast::Stmt,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &stmt.kind {
        ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } => dependency_import_occurrence_in_pattern(pattern, offset)
            .or_else(|| {
                ty.as_ref()
                    .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
            })
            .or_else(|| dependency_import_occurrence_in_expr(value, offset)),
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => dependency_import_occurrence_in_expr(expr, offset),
        ql_ast::StmtKind::While { condition, body } => {
            dependency_import_occurrence_in_expr(condition, offset)
                .or_else(|| dependency_import_occurrence_in_block(body, offset))
        }
        ql_ast::StmtKind::Loop { body } => dependency_import_occurrence_in_block(body, offset),
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => dependency_import_occurrence_in_pattern(pattern, offset)
            .or_else(|| dependency_import_occurrence_in_expr(iterable, offset))
            .or_else(|| dependency_import_occurrence_in_block(body, offset)),
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn dependency_type_import_occurrence_in_stmt(
    stmt: &ql_ast::Stmt,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &stmt.kind {
        ql_ast::StmtKind::Let { ty, value, .. } => ty
            .as_ref()
            .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
            .or_else(|| dependency_type_import_occurrence_in_expr(value, offset)),
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            dependency_type_import_occurrence_in_expr(expr, offset)
        }
        ql_ast::StmtKind::While { condition, body } => {
            dependency_type_import_occurrence_in_expr(condition, offset)
                .or_else(|| dependency_type_import_occurrence_in_block(body, offset))
        }
        ql_ast::StmtKind::Loop { body } => dependency_type_import_occurrence_in_block(body, offset),
        ql_ast::StmtKind::For { iterable, body, .. } => {
            dependency_type_import_occurrence_in_expr(iterable, offset)
                .or_else(|| dependency_type_import_occurrence_in_block(body, offset))
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn dependency_import_occurrence_in_type_expr(
    ty: &ql_ast::TypeExpr,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &ty.kind {
        ql_ast::TypeExprKind::Pointer { inner, .. } => {
            dependency_import_occurrence_in_type_expr(inner, offset)
        }
        ql_ast::TypeExprKind::Array { element, .. } => {
            dependency_import_occurrence_in_type_expr(element, offset)
        }
        ql_ast::TypeExprKind::Named { path, args } => {
            dependency_import_occurrence_for_path(path, offset).or_else(|| {
                args.iter()
                    .find_map(|arg| dependency_import_occurrence_in_type_expr(arg, offset))
            })
        }
        ql_ast::TypeExprKind::Tuple(items) => items
            .iter()
            .find_map(|item| dependency_import_occurrence_in_type_expr(item, offset)),
        ql_ast::TypeExprKind::Callable { params, ret } => params
            .iter()
            .find_map(|param| dependency_import_occurrence_in_type_expr(param, offset))
            .or_else(|| dependency_import_occurrence_in_type_expr(ret, offset)),
    }
}

fn dependency_type_import_occurrence_in_expr(
    expr: &ql_ast::Expr,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &expr.kind {
        ql_ast::ExprKind::Name(_)
        | ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => items
            .iter()
            .find_map(|item| dependency_type_import_occurrence_in_expr(item, offset)),
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_type_import_occurrence_in_block(block, offset)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_type_import_occurrence_in_expr(condition, offset)
            .or_else(|| dependency_type_import_occurrence_in_block(then_branch, offset))
            .or_else(|| {
                else_branch
                    .as_ref()
                    .and_then(|expr| dependency_type_import_occurrence_in_expr(expr, offset))
            }),
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_type_import_occurrence_in_expr(value, offset).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(|guard| dependency_type_import_occurrence_in_expr(guard, offset))
                        .or_else(|| dependency_type_import_occurrence_in_expr(&arm.body, offset))
                })
            })
        }
        ql_ast::ExprKind::Closure { params, body, .. } => params
            .iter()
            .find_map(|param| {
                param
                    .ty
                    .as_ref()
                    .and_then(|ty| dependency_import_occurrence_in_type_expr(ty, offset))
            })
            .or_else(|| dependency_type_import_occurrence_in_expr(body, offset)),
        ql_ast::ExprKind::Call { callee, args } => {
            dependency_type_import_occurrence_in_expr(callee, offset).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        dependency_type_import_occurrence_in_expr(expr, offset)
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        dependency_type_import_occurrence_in_expr(value, offset)
                    }
                })
            })
        }
        ql_ast::ExprKind::Member { object, .. } => {
            dependency_type_import_occurrence_in_expr(object, offset)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            dependency_type_import_occurrence_in_expr(target, offset).or_else(|| {
                items
                    .iter()
                    .find_map(|item| dependency_type_import_occurrence_in_expr(item, offset))
            })
        }
        ql_ast::ExprKind::StructLiteral { fields, .. } => fields.iter().find_map(|field| {
            field
                .value
                .as_ref()
                .and_then(|value| dependency_type_import_occurrence_in_expr(value, offset))
        }),
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_type_import_occurrence_in_expr(left, offset)
                .or_else(|| dependency_type_import_occurrence_in_expr(right, offset))
        }
        ql_ast::ExprKind::Unary { expr, .. } | ql_ast::ExprKind::Question(expr) => {
            dependency_type_import_occurrence_in_expr(expr, offset)
        }
    }
}

fn dependency_import_occurrence_in_pattern(
    pattern: &ql_ast::Pattern,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &pattern.kind {
        ql_ast::PatternKind::Tuple(items) | ql_ast::PatternKind::Array(items) => items
            .iter()
            .find_map(|item| dependency_import_occurrence_in_pattern(item, offset)),
        ql_ast::PatternKind::Path(path) => dependency_import_occurrence_for_path(path, offset),
        ql_ast::PatternKind::TupleStruct { path, items } => {
            dependency_import_occurrence_for_path(path, offset).or_else(|| {
                items
                    .iter()
                    .find_map(|item| dependency_import_occurrence_in_pattern(item, offset))
            })
        }
        ql_ast::PatternKind::Struct { path, fields, .. } => {
            dependency_import_occurrence_for_path(path, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field.pattern.as_ref().and_then(|pattern| {
                        dependency_import_occurrence_in_pattern(pattern, offset)
                    })
                })
            })
        }
        ql_ast::PatternKind::Name(_)
        | ql_ast::PatternKind::Integer(_)
        | ql_ast::PatternKind::String(_)
        | ql_ast::PatternKind::Bool(_)
        | ql_ast::PatternKind::NoneLiteral
        | ql_ast::PatternKind::Wildcard => None,
    }
}

fn dependency_import_occurrence_in_expr(
    expr: &ql_ast::Expr,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    match &expr.kind {
        ql_ast::ExprKind::Name(name) => dependency_struct_field_completion_span_contains(
            expr.span, offset,
        )
        .then_some(DependencyImportOccurrence {
            local_name: name.clone(),
            span: expr.span,
            is_definition: false,
        }),
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => items
            .iter()
            .find_map(|item| dependency_import_occurrence_in_expr(item, offset)),
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_import_occurrence_in_block(block, offset)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_import_occurrence_in_expr(condition, offset)
            .or_else(|| dependency_import_occurrence_in_block(then_branch, offset))
            .or_else(|| {
                else_branch
                    .as_ref()
                    .and_then(|expr| dependency_import_occurrence_in_expr(expr, offset))
            }),
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_import_occurrence_in_expr(value, offset).or_else(|| {
                arms.iter().find_map(|arm| {
                    dependency_import_occurrence_in_pattern(&arm.pattern, offset)
                        .or_else(|| {
                            arm.guard.as_ref().and_then(|guard| {
                                dependency_import_occurrence_in_expr(guard, offset)
                            })
                        })
                        .or_else(|| dependency_import_occurrence_in_expr(&arm.body, offset))
                })
            })
        }
        ql_ast::ExprKind::Closure { body, .. } => {
            dependency_import_occurrence_in_expr(body, offset)
        }
        ql_ast::ExprKind::Call { callee, args } => {
            dependency_import_occurrence_in_expr(callee, offset).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        dependency_import_occurrence_in_expr(expr, offset)
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        dependency_import_occurrence_in_expr(value, offset)
                    }
                })
            })
        }
        ql_ast::ExprKind::Member { object, .. } | ql_ast::ExprKind::Question(object) => {
            dependency_import_occurrence_in_expr(object, offset)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            dependency_import_occurrence_in_expr(target, offset).or_else(|| {
                items
                    .iter()
                    .find_map(|item| dependency_import_occurrence_in_expr(item, offset))
            })
        }
        ql_ast::ExprKind::StructLiteral { path, fields } => {
            dependency_import_occurrence_for_path(path, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field
                        .value
                        .as_ref()
                        .and_then(|value| dependency_import_occurrence_in_expr(value, offset))
                })
            })
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_import_occurrence_in_expr(left, offset)
                .or_else(|| dependency_import_occurrence_in_expr(right, offset))
        }
        ql_ast::ExprKind::Unary { expr, .. } => dependency_import_occurrence_in_expr(expr, offset),
        ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
    }
}

fn dependency_import_occurrence_for_path(
    path: &ql_ast::Path,
    offset: usize,
) -> Option<DependencyImportOccurrence> {
    let [local_name] = path.segments.as_slice() else {
        return None;
    };
    let span = path.first_segment_span()?;
    dependency_struct_field_completion_span_contains(span, offset).then_some(
        DependencyImportOccurrence {
            local_name: local_name.clone(),
            span,
            is_definition: false,
        },
    )
}

fn dependency_value_root_binding_for_path(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    path: &ql_ast::Path,
    offset: usize,
) -> Option<DependencyStructBinding> {
    let Some(root_name) = path.segments.first() else {
        return None;
    };
    let span = path.first_segment_span()?;
    dependency_struct_field_completion_span_contains(span, offset)
        .then(|| dependency_struct_binding_for_local_name(package, module, root_name))
        .flatten()
}

fn push_dependency_value_root_occurrence_for_path(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    path: &ql_ast::Path,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    let Some(root_name) = path.segments.first() else {
        return;
    };
    let Some(reference_span) = path.first_segment_span() else {
        return;
    };
    let Some(binding) = dependency_struct_binding_for_local_name(package, module, root_name) else {
        return;
    };
    push_dependency_value_root_occurrence(
        SymbolKind::Struct,
        root_name,
        reference_span,
        &binding,
        occurrences,
    );
}

fn collect_dependency_method_occurrences_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    match &item.kind {
        AstItemKind::Function(function) => {
            if let Some(body) = &function.body {
                scopes.push(HashMap::new());
                iterable_scopes.push(HashMap::new());
                for param in &function.params {
                    bind_dependency_struct_param(package, module, param, None, scopes);
                    bind_dependency_iterable_param(param, iterable_scopes);
                }
                collect_dependency_method_occurrences_in_block(
                    package,
                    module,
                    body,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
                iterable_scopes.pop();
                scopes.pop();
            }
        }
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                &global.value,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        AstItemKind::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                if let Some(default) = &field.default {
                    collect_dependency_method_occurrences_in_expr(
                        package,
                        module,
                        default,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
        }
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    iterable_scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(package, module, param, None, scopes);
                        bind_dependency_iterable_param(param, iterable_scopes);
                    }
                    collect_dependency_method_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    iterable_scopes.pop();
                    scopes.pop();
                }
            }
        }
        AstItemKind::Impl(impl_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &impl_block.target);
            for method in &impl_block.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    iterable_scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                        bind_dependency_iterable_param(param, iterable_scopes);
                    }
                    collect_dependency_method_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    iterable_scopes.pop();
                    scopes.pop();
                }
            }
        }
        AstItemKind::Extend(extend_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &extend_block.target);
            for method in &extend_block.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    iterable_scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                        bind_dependency_iterable_param(param, iterable_scopes);
                    }
                    collect_dependency_method_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    iterable_scopes.pop();
                    scopes.pop();
                }
            }
        }
        AstItemKind::TypeAlias(_) | AstItemKind::Enum(_) | AstItemKind::ExternBlock(_) => {}
    }
}

fn collect_dependency_method_occurrences_in_block(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    scopes.push(HashMap::new());
    iterable_scopes.push(HashMap::new());
    for stmt in &block.statements {
        collect_dependency_method_occurrences_in_stmt(
            package,
            module,
            stmt,
            scopes,
            iterable_scopes,
            occurrences,
        );
    }
    if let Some(tail) = &block.tail {
        collect_dependency_method_occurrences_in_expr(
            package,
            module,
            tail,
            scopes,
            iterable_scopes,
            occurrences,
        );
    }
    iterable_scopes.pop();
    scopes.pop();
}

fn collect_dependency_method_occurrences_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    match &stmt.kind {
        ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                value,
                scopes,
                iterable_scopes,
                occurrences,
            );
            bind_dependency_struct_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                scopes,
                iterable_scopes,
            );
            bind_dependency_iterable_let(package, module, pattern, value, scopes, iterable_scopes);
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::While { condition, body } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                condition,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Loop { body } => {
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            let iterable_binding = dependency_struct_element_binding_for_iterable_expr(
                package,
                module,
                iterable,
                scopes,
                iterable_scopes,
            );
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                iterable,
                scopes,
                iterable_scopes,
                occurrences,
            );
            scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            if let Some(binding) = &iterable_binding {
                bind_dependency_struct_pattern(package, pattern, binding, scopes);
            }
            shadow_dependency_iterable_pattern(pattern, iterable_scopes);
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
            iterable_scopes.pop();
            scopes.pop();
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {}
    }
}

fn collect_dependency_method_occurrences_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            for item in items {
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    item,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                block,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                condition,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                then_branch,
                scopes,
                iterable_scopes,
                occurrences,
            );
            if let Some(expr) = else_branch {
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    expr,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Match { value, arms } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                value,
                scopes,
                iterable_scopes,
                occurrences,
            );
            let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
                iterable_scopes.push(HashMap::new());
                if let Some(binding) = &value_binding {
                    bind_dependency_struct_match_pattern(package, &arm.pattern, binding, scopes);
                }
                shadow_dependency_iterable_pattern(&arm.pattern, iterable_scopes);
                if let Some(guard) = &arm.guard {
                    collect_dependency_method_occurrences_in_expr(
                        package,
                        module,
                        guard,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    &arm.body,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
                iterable_scopes.pop();
                scopes.pop();
            }
        }
        ql_ast::ExprKind::Closure { params, body, .. } => {
            scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            for param in params {
                bind_dependency_struct_closure_param(package, module, param, scopes);
                bind_dependency_iterable_closure_param(param, iterable_scopes);
            }
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
            iterable_scopes.pop();
            scopes.pop();
        }
        ql_ast::ExprKind::Call { callee, args } => {
            match &callee.kind {
                ql_ast::ExprKind::Member {
                    object,
                    field,
                    field_span,
                } => {
                    collect_dependency_method_occurrences_in_expr(
                        package,
                        module,
                        object,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    if let Some(binding) =
                        dependency_struct_binding_for_expr(package, module, object, scopes)
                    {
                        push_dependency_method_occurrence_for_binding(
                            &binding,
                            field,
                            *field_span,
                            occurrences,
                        );
                    }
                }
                _ => {
                    collect_dependency_method_occurrences_in_expr(
                        package,
                        module,
                        callee,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
            for arg in args {
                match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        collect_dependency_method_occurrences_in_expr(
                            package,
                            module,
                            expr,
                            scopes,
                            iterable_scopes,
                            occurrences,
                        );
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        collect_dependency_method_occurrences_in_expr(
                            package,
                            module,
                            value,
                            scopes,
                            iterable_scopes,
                            occurrences,
                        );
                    }
                }
            }
        }
        ql_ast::ExprKind::Member { object, .. } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                object,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                target,
                scopes,
                iterable_scopes,
                occurrences,
            );
            for item in items {
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    item,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::StructLiteral { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_dependency_method_occurrences_in_expr(
                        package,
                        module,
                        value,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                left,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                right,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Unary { expr, .. } | ql_ast::ExprKind::Question(expr) => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Name(_)
        | ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => {}
    }
}

fn collect_dependency_value_occurrences_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    binding_scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    value_scopes: &mut Vec<HashMap<String, DependencyValueBinding>>,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    match &item.kind {
        AstItemKind::Function(function) => collect_dependency_value_occurrences_in_function(
            package,
            module,
            function,
            None,
            binding_scopes,
            iterable_scopes,
            value_scopes,
            occurrences,
        ),
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                &global.value,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        AstItemKind::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                if let Some(default) = &field.default {
                    collect_dependency_value_occurrences_in_expr(
                        package,
                        module,
                        default,
                        binding_scopes,
                        iterable_scopes,
                        value_scopes,
                        occurrences,
                    );
                }
            }
        }
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                collect_dependency_value_occurrences_in_function(
                    package,
                    module,
                    method,
                    None,
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        AstItemKind::Impl(impl_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &impl_block.target);
            for method in &impl_block.methods {
                collect_dependency_value_occurrences_in_function(
                    package,
                    module,
                    method,
                    receiver_binding.as_ref(),
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        AstItemKind::Extend(extend_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &extend_block.target);
            for method in &extend_block.methods {
                collect_dependency_value_occurrences_in_function(
                    package,
                    module,
                    method,
                    receiver_binding.as_ref(),
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        AstItemKind::TypeAlias(_) | AstItemKind::Enum(_) | AstItemKind::ExternBlock(_) => {}
    }
}

fn collect_dependency_value_occurrences_in_function(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    function: &ql_ast::FunctionDecl,
    receiver_binding: Option<&DependencyStructBinding>,
    binding_scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    value_scopes: &mut Vec<HashMap<String, DependencyValueBinding>>,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    let Some(body) = &function.body else {
        return;
    };
    binding_scopes.push(HashMap::new());
    iterable_scopes.push(HashMap::new());
    value_scopes.push(HashMap::new());
    for param in &function.params {
        bind_dependency_value_param(
            package,
            module,
            param,
            receiver_binding,
            binding_scopes,
            value_scopes,
            occurrences,
        );
        bind_dependency_iterable_param(param, iterable_scopes);
    }
    collect_dependency_value_occurrences_in_block(
        package,
        module,
        body,
        binding_scopes,
        iterable_scopes,
        value_scopes,
        occurrences,
    );
    value_scopes.pop();
    iterable_scopes.pop();
    binding_scopes.pop();
}

fn collect_dependency_value_occurrences_in_block(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    binding_scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    value_scopes: &mut Vec<HashMap<String, DependencyValueBinding>>,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    binding_scopes.push(HashMap::new());
    iterable_scopes.push(HashMap::new());
    value_scopes.push(HashMap::new());
    for stmt in &block.statements {
        collect_dependency_value_occurrences_in_stmt(
            package,
            module,
            stmt,
            binding_scopes,
            iterable_scopes,
            value_scopes,
            occurrences,
        );
    }
    if let Some(tail) = &block.tail {
        collect_dependency_value_occurrences_in_expr(
            package,
            module,
            tail,
            binding_scopes,
            iterable_scopes,
            value_scopes,
            occurrences,
        );
    }
    value_scopes.pop();
    iterable_scopes.pop();
    binding_scopes.pop();
}

fn collect_dependency_value_occurrences_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    binding_scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    value_scopes: &mut Vec<HashMap<String, DependencyValueBinding>>,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    match &stmt.kind {
        ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                value,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            bind_dependency_value_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            bind_dependency_iterable_let(
                package,
                module,
                pattern,
                value,
                binding_scopes,
                iterable_scopes,
            );
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                expr,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::While { condition, body } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                condition,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            collect_dependency_value_occurrences_in_block(
                package,
                module,
                body,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Loop { body } => {
            collect_dependency_value_occurrences_in_block(
                package,
                module,
                body,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            let iterable_binding = dependency_struct_element_binding_for_iterable_expr(
                package,
                module,
                iterable,
                binding_scopes,
                iterable_scopes,
            );
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                iterable,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            binding_scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            value_scopes.push(HashMap::new());
            if let Some(binding) = &iterable_binding {
                bind_dependency_value_pattern(
                    package,
                    pattern,
                    binding,
                    binding_scopes,
                    value_scopes,
                    occurrences,
                );
            }
            shadow_dependency_iterable_pattern(pattern, iterable_scopes);
            collect_dependency_value_occurrences_in_block(
                package,
                module,
                body,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            value_scopes.pop();
            iterable_scopes.pop();
            binding_scopes.pop();
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {}
    }
}

fn collect_dependency_value_occurrences_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    binding_scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    value_scopes: &mut Vec<HashMap<String, DependencyValueBinding>>,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            for item in items {
                collect_dependency_value_occurrences_in_expr(
                    package,
                    module,
                    item,
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            collect_dependency_value_occurrences_in_block(
                package,
                module,
                block,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                condition,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            collect_dependency_value_occurrences_in_block(
                package,
                module,
                then_branch,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            if let Some(expr) = else_branch {
                collect_dependency_value_occurrences_in_expr(
                    package,
                    module,
                    expr,
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Match { value, arms } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                value,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            let value_binding =
                dependency_struct_binding_for_expr(package, module, value, binding_scopes);
            for arm in arms {
                binding_scopes.push(HashMap::new());
                iterable_scopes.push(HashMap::new());
                value_scopes.push(HashMap::new());
                if let Some(binding) = &value_binding {
                    bind_dependency_value_match_pattern(
                        package,
                        &arm.pattern,
                        binding,
                        binding_scopes,
                        value_scopes,
                        occurrences,
                    );
                }
                shadow_dependency_iterable_pattern(&arm.pattern, iterable_scopes);
                if let Some(guard) = &arm.guard {
                    collect_dependency_value_occurrences_in_expr(
                        package,
                        module,
                        guard,
                        binding_scopes,
                        iterable_scopes,
                        value_scopes,
                        occurrences,
                    );
                }
                collect_dependency_value_occurrences_in_expr(
                    package,
                    module,
                    &arm.body,
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
                value_scopes.pop();
                iterable_scopes.pop();
                binding_scopes.pop();
            }
        }
        ql_ast::ExprKind::Closure { params, body, .. } => {
            binding_scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            value_scopes.push(HashMap::new());
            for param in params {
                bind_dependency_value_closure_param(
                    package,
                    module,
                    param,
                    binding_scopes,
                    value_scopes,
                    occurrences,
                );
                bind_dependency_iterable_closure_param(param, iterable_scopes);
            }
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                body,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            value_scopes.pop();
            iterable_scopes.pop();
            binding_scopes.pop();
        }
        ql_ast::ExprKind::Call { callee, args } => {
            if let ql_ast::ExprKind::Member {
                field, field_span, ..
            } = &callee.kind
                && let Some(binding) =
                    dependency_struct_binding_for_call_expr(package, module, callee, binding_scopes)
            {
                push_dependency_value_root_occurrence(
                    SymbolKind::Method,
                    field,
                    *field_span,
                    &binding,
                    occurrences,
                );
            }
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                callee,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            for arg in args {
                match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        collect_dependency_value_occurrences_in_expr(
                            package,
                            module,
                            expr,
                            binding_scopes,
                            iterable_scopes,
                            value_scopes,
                            occurrences,
                        );
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        collect_dependency_value_occurrences_in_expr(
                            package,
                            module,
                            value,
                            binding_scopes,
                            iterable_scopes,
                            value_scopes,
                            occurrences,
                        );
                    }
                }
            }
        }
        ql_ast::ExprKind::Member { object, .. } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                object,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Question(inner) => {
            match &inner.kind {
                ql_ast::ExprKind::Member {
                    field, field_span, ..
                } => {
                    if let Some(binding) = dependency_struct_binding_for_question_expr(
                        package,
                        module,
                        inner,
                        binding_scopes,
                    ) {
                        push_dependency_value_root_occurrence(
                            SymbolKind::Field,
                            field,
                            *field_span,
                            &binding,
                            occurrences,
                        );
                    }
                }
                ql_ast::ExprKind::Call { callee, .. } => {
                    if let ql_ast::ExprKind::Member {
                        field, field_span, ..
                    } = &callee.kind
                        && let Some(binding) = dependency_struct_binding_for_question_expr(
                            package,
                            module,
                            inner,
                            binding_scopes,
                        )
                    {
                        push_dependency_value_root_occurrence(
                            SymbolKind::Method,
                            field,
                            *field_span,
                            &binding,
                            occurrences,
                        );
                    }
                }
                _ => {}
            }
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                inner,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            push_dependency_indexed_value_root_occurrence_for_bracket_target(
                package,
                module,
                target,
                binding_scopes,
                occurrences,
            );
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                target,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            for item in items {
                collect_dependency_value_occurrences_in_expr(
                    package,
                    module,
                    item,
                    binding_scopes,
                    iterable_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::StructLiteral { path, fields } => {
            push_dependency_value_root_occurrence_for_path(package, module, path, occurrences);
            for field in fields {
                if let Some(value) = &field.value {
                    collect_dependency_value_occurrences_in_expr(
                        package,
                        module,
                        value,
                        binding_scopes,
                        iterable_scopes,
                        value_scopes,
                        occurrences,
                    );
                }
            }
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                left,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                right,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Unary { expr, .. } => {
            collect_dependency_value_occurrences_in_expr(
                package,
                module,
                expr,
                binding_scopes,
                iterable_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Name(name) => {
            let Some(binding) = dependency_value_binding_for_name(value_scopes, name) else {
                return;
            };
            push_dependency_value_occurrence(binding, expr.span, false, occurrences);
        }
        ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => {}
    }
}

fn collect_dependency_struct_field_occurrences_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    match &item.kind {
        AstItemKind::Function(function) => {
            if let Some(body) = &function.body {
                scopes.push(HashMap::new());
                iterable_scopes.push(HashMap::new());
                for param in &function.params {
                    bind_dependency_struct_param(package, module, param, None, scopes);
                    bind_dependency_iterable_param(param, iterable_scopes);
                }
                collect_dependency_struct_field_occurrences_in_block(
                    package,
                    module,
                    body,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
                iterable_scopes.pop();
                scopes.pop();
            }
        }
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                &global.value,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        AstItemKind::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                if let Some(default) = &field.default {
                    collect_dependency_struct_field_occurrences_in_expr(
                        package,
                        module,
                        default,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
        }
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    iterable_scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(package, module, param, None, scopes);
                        bind_dependency_iterable_param(param, iterable_scopes);
                    }
                    collect_dependency_struct_field_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    iterable_scopes.pop();
                    scopes.pop();
                }
            }
        }
        AstItemKind::Impl(impl_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &impl_block.target);
            for method in &impl_block.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    iterable_scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                        bind_dependency_iterable_param(param, iterable_scopes);
                    }
                    collect_dependency_struct_field_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    iterable_scopes.pop();
                    scopes.pop();
                }
            }
        }
        AstItemKind::Extend(extend_block) => {
            let receiver_binding =
                dependency_struct_binding_for_type_expr(package, module, &extend_block.target);
            for method in &extend_block.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    iterable_scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                        bind_dependency_iterable_param(param, iterable_scopes);
                    }
                    collect_dependency_struct_field_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                    iterable_scopes.pop();
                    scopes.pop();
                }
            }
        }
        AstItemKind::TypeAlias(_) | AstItemKind::Enum(_) | AstItemKind::ExternBlock(_) => {}
    }
}

fn collect_dependency_struct_field_occurrences_in_block(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    scopes.push(HashMap::new());
    iterable_scopes.push(HashMap::new());
    for stmt in &block.statements {
        collect_dependency_struct_field_occurrences_in_stmt(
            package,
            module,
            stmt,
            scopes,
            iterable_scopes,
            occurrences,
        );
    }
    if let Some(tail) = &block.tail {
        collect_dependency_struct_field_occurrences_in_expr(
            package,
            module,
            tail,
            scopes,
            iterable_scopes,
            occurrences,
        );
    }
    iterable_scopes.pop();
    scopes.pop();
}

fn collect_dependency_struct_field_occurrences_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    match &stmt.kind {
        ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } => {
            collect_dependency_struct_field_occurrences_in_pattern(
                package,
                module,
                pattern,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                value,
                scopes,
                iterable_scopes,
                occurrences,
            );
            bind_dependency_struct_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                scopes,
                iterable_scopes,
            );
            bind_dependency_iterable_let(package, module, pattern, value, scopes, iterable_scopes);
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::While { condition, body } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                condition,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Loop { body } => {
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            let iterable_binding = dependency_struct_element_binding_for_iterable_expr(
                package,
                module,
                iterable,
                scopes,
                iterable_scopes,
            );
            collect_dependency_struct_field_occurrences_in_pattern(
                package,
                module,
                pattern,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                iterable,
                scopes,
                iterable_scopes,
                occurrences,
            );
            scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            if let Some(binding) = &iterable_binding {
                bind_dependency_struct_pattern(package, pattern, binding, scopes);
            }
            shadow_dependency_iterable_pattern(pattern, iterable_scopes);
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
            iterable_scopes.pop();
            scopes.pop();
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {}
    }
}

fn collect_dependency_struct_field_occurrences_in_pattern(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    pattern: &ql_ast::Pattern,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    match &pattern.kind {
        ql_ast::PatternKind::Tuple(items)
        | ql_ast::PatternKind::Array(items)
        | ql_ast::PatternKind::TupleStruct { items, .. } => {
            for item in items {
                collect_dependency_struct_field_occurrences_in_pattern(
                    package,
                    module,
                    item,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::PatternKind::Struct { path, fields, .. } => {
            for field in fields {
                push_dependency_struct_field_occurrence_for_path(
                    package,
                    module,
                    path,
                    &field.name,
                    field.name_span,
                    occurrences,
                );
                if let Some(pattern) = &field.pattern {
                    collect_dependency_struct_field_occurrences_in_pattern(
                        package,
                        module,
                        pattern,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
        }
        ql_ast::PatternKind::Name(_)
        | ql_ast::PatternKind::Path(_)
        | ql_ast::PatternKind::Integer(_)
        | ql_ast::PatternKind::String(_)
        | ql_ast::PatternKind::Bool(_)
        | ql_ast::PatternKind::NoneLiteral
        | ql_ast::PatternKind::Wildcard => {}
    }
}

fn collect_dependency_struct_field_occurrences_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    iterable_scopes: &mut DependencyIterableScopes,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            for item in items {
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    item,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                block,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                condition,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                then_branch,
                scopes,
                iterable_scopes,
                occurrences,
            );
            if let Some(expr) = else_branch {
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    expr,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Match { value, arms } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                value,
                scopes,
                iterable_scopes,
                occurrences,
            );
            let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
                iterable_scopes.push(HashMap::new());
                if let Some(binding) = &value_binding {
                    bind_dependency_struct_match_pattern(package, &arm.pattern, binding, scopes);
                }
                shadow_dependency_iterable_pattern(&arm.pattern, iterable_scopes);
                collect_dependency_struct_field_occurrences_in_pattern(
                    package,
                    module,
                    &arm.pattern,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
                if let Some(guard) = &arm.guard {
                    collect_dependency_struct_field_occurrences_in_expr(
                        package,
                        module,
                        guard,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    &arm.body,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
                iterable_scopes.pop();
                scopes.pop();
            }
        }
        ql_ast::ExprKind::Closure { params, body, .. } => {
            scopes.push(HashMap::new());
            iterable_scopes.push(HashMap::new());
            for param in params {
                bind_dependency_struct_closure_param(package, module, param, scopes);
                bind_dependency_iterable_closure_param(param, iterable_scopes);
            }
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                body,
                scopes,
                iterable_scopes,
                occurrences,
            );
            iterable_scopes.pop();
            scopes.pop();
        }
        ql_ast::ExprKind::Call { callee, args } => {
            match &callee.kind {
                ql_ast::ExprKind::Member { object, .. } => {
                    collect_dependency_struct_field_occurrences_in_expr(
                        package,
                        module,
                        object,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
                _ => {
                    collect_dependency_struct_field_occurrences_in_expr(
                        package,
                        module,
                        callee,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
            for arg in args {
                match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        collect_dependency_struct_field_occurrences_in_expr(
                            package,
                            module,
                            expr,
                            scopes,
                            iterable_scopes,
                            occurrences,
                        );
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        collect_dependency_struct_field_occurrences_in_expr(
                            package,
                            module,
                            value,
                            scopes,
                            iterable_scopes,
                            occurrences,
                        );
                    }
                }
            }
        }
        ql_ast::ExprKind::Member {
            object,
            field,
            field_span,
        } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                object,
                scopes,
                iterable_scopes,
                occurrences,
            );
            if let Some(binding) =
                dependency_struct_binding_for_expr(package, module, object, scopes)
            {
                push_dependency_struct_field_occurrence_for_binding(
                    &binding,
                    field,
                    *field_span,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Question(object) => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                object,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                target,
                scopes,
                iterable_scopes,
                occurrences,
            );
            for item in items {
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    item,
                    scopes,
                    iterable_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::StructLiteral { path, fields } => {
            for field in fields {
                push_dependency_struct_field_occurrence_for_path(
                    package,
                    module,
                    path,
                    &field.name,
                    field.name_span,
                    occurrences,
                );
                if let Some(value) = &field.value {
                    collect_dependency_struct_field_occurrences_in_expr(
                        package,
                        module,
                        value,
                        scopes,
                        iterable_scopes,
                        occurrences,
                    );
                }
            }
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                left,
                scopes,
                iterable_scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                right,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Unary { expr, .. } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
                iterable_scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Name(_)
        | ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => {}
    }
}

fn push_dependency_struct_field_occurrence_for_path(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    path: &ql_ast::Path,
    field_name: &str,
    field_span: Span,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    let Some(root_name) = path.segments.first() else {
        return;
    };
    let Some((dependency, symbol)) =
        dependency_struct_import_binding_for_local_name(package, module, root_name)
    else {
        return;
    };
    let Some(field) = dependency.struct_field_for(symbol, field_name) else {
        return;
    };
    let Some(definition_span) =
        dependency.artifact_source_span(&symbol.source_path, field.name_span)
    else {
        return;
    };
    occurrences.push(DependencyStructFieldOccurrence {
        reference_span: field_span,
        package_name: dependency.artifact.package_name.clone(),
        manifest_path: dependency.manifest.manifest_path.clone(),
        source_path: symbol.source_path.clone(),
        struct_name: symbol.name.clone(),
        name: field.name.clone(),
        detail: dependency_struct_field_detail(field),
        path: dependency.interface_path.clone(),
        definition_span,
    });
}

fn dependency_struct_import_binding_for_local_name<'a>(
    package: &'a PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<(&'a DependencyInterface, &'a DependencySymbol)> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    (symbol.kind == SymbolKind::Struct).then_some((dependency, symbol))
}

fn dependency_struct_binding_for_symbol(
    dependency: &DependencyInterface,
    symbol: &DependencySymbol,
) -> Option<DependencyStructBinding> {
    let struct_decl = dependency.struct_decl_for(symbol)?;
    let definition_span = dependency.artifact_span_for(symbol)?;
    let fields = struct_decl
        .fields
        .iter()
        .filter_map(|field| {
            let definition_span =
                dependency.artifact_source_span(&symbol.source_path, field.name_span)?;
            Some((
                field.name.clone(),
                DependencyStructResolvedField {
                    name: field.name.clone(),
                    detail: dependency_struct_field_detail(field),
                    ty: render_dependency_type_expr(&field.ty),
                    definition_span,
                    type_definition: dependency.public_type_target_for_type_expr(&field.ty),
                    question_type_definition: dependency
                        .public_question_inner_type_target_for_type_expr(&field.ty),
                    iterable_element_type_definition: dependency
                        .public_iterable_element_type_target_for_type_expr(&field.ty),
                    question_iterable_element_type_definition: dependency
                        .public_question_inner_iterable_element_type_target_for_type_expr(
                            &field.ty,
                        ),
                },
            ))
        })
        .collect();
    let methods = dependency.struct_methods_for(symbol);

    Some(DependencyStructBinding {
        package_name: dependency.artifact.package_name.clone(),
        manifest_path: dependency.manifest.manifest_path.clone(),
        source_path: symbol.source_path.clone(),
        struct_name: symbol.name.clone(),
        detail: symbol.detail.clone(),
        path: dependency.interface_path.clone(),
        definition_span,
        fields,
        methods,
    })
}

fn dependency_struct_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_struct_import_binding_for_local_name(package, module, local_name)?;
    dependency_struct_binding_for_symbol(dependency, symbol)
}

fn dependency_function_return_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.function_return_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_function_question_return_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.function_question_return_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_function_iterable_element_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.function_iterable_element_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_function_question_iterable_element_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.function_question_iterable_element_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_global_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.global_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_global_question_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.global_question_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_value_root_import_binding_matches(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
    target: &DependencyStructBinding,
) -> bool {
    [
        dependency_function_return_binding_for_local_name(package, module, local_name),
        dependency_function_question_return_binding_for_local_name(package, module, local_name),
        dependency_function_iterable_element_binding_for_local_name(package, module, local_name),
        dependency_function_question_iterable_element_binding_for_local_name(
            package, module, local_name,
        ),
        dependency_global_binding_for_local_name(package, module, local_name),
        dependency_global_question_binding_for_local_name(package, module, local_name),
        dependency_global_iterable_element_binding_for_local_name(package, module, local_name),
        dependency_global_question_iterable_element_binding_for_local_name(
            package, module, local_name,
        ),
    ]
    .into_iter()
    .flatten()
    .any(|binding| dependency_struct_bindings_match(&binding, target))
}

fn dependency_global_iterable_element_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.global_iterable_element_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_global_question_iterable_element_binding_for_local_name(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<DependencyStructBinding> {
    let (dependency, symbol) =
        dependency_import_binding_for_local_name(package, module, local_name)?;
    let target = dependency.global_question_iterable_element_type_target(symbol)?;
    dependency_struct_binding_for_definition_target(package, &target)
}

fn dependency_struct_binding_for_type_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyStructBinding> {
    let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
        return None;
    };
    let [root_name] = path.segments.as_slice() else {
        return None;
    };
    dependency_struct_binding_for_local_name(package, module, root_name)
}

fn dependency_struct_common_binding_for_type_exprs(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    items: &[ql_ast::TypeExpr],
) -> Option<DependencyStructBinding> {
    let mut items = items.iter();
    let first = dependency_struct_binding_for_type_expr(package, module, items.next()?)?;
    for item in items {
        let binding = dependency_struct_binding_for_type_expr(package, module, item)?;
        if binding != first {
            return None;
        }
    }
    Some(first)
}

fn dependency_struct_element_binding_for_type_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyStructBinding> {
    match &ty.kind {
        ql_ast::TypeExprKind::Array { element, .. } => {
            dependency_struct_binding_for_type_expr(package, module, element)
        }
        ql_ast::TypeExprKind::Tuple(items) => {
            dependency_struct_common_binding_for_type_exprs(package, module, items)
        }
        _ => None,
    }
}

fn dependency_struct_question_element_binding_for_type_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyStructBinding> {
    let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    let [type_name] = path.segments.as_slice() else {
        return None;
    };
    let inner = match (type_name.as_str(), args.as_slice()) {
        ("Option", [inner]) => inner,
        ("Result", [inner, ..]) => inner,
        _ => return None,
    };
    dependency_struct_element_binding_for_type_expr(package, module, inner).or_else(|| {
        dependency_struct_question_element_binding_for_type_expr(package, module, inner)
    })
}

fn dependency_struct_question_binding_for_type_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    ty: &ql_ast::TypeExpr,
) -> Option<DependencyStructBinding> {
    let ql_ast::TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    let [type_name] = path.segments.as_slice() else {
        return None;
    };
    let inner = match (type_name.as_str(), args.as_slice()) {
        ("Option", [inner]) => inner,
        ("Result", [inner, ..]) => inner,
        _ => return None,
    };
    dependency_struct_binding_for_type_expr(package, module, inner)
        .or_else(|| dependency_struct_question_binding_for_type_expr(package, module, inner))
}

fn local_function_decl_for_name<'a>(
    module: &'a ql_ast::Module,
    name: &str,
) -> Option<&'a ql_ast::FunctionDecl> {
    module.items.iter().find_map(|item| match &item.kind {
        AstItemKind::Function(function) if function.name == name => Some(function),
        _ => None,
    })
}

fn local_receiver_method_decl_for_name<'a>(
    package: &PackageAnalysis,
    module: &'a ql_ast::Module,
    receiver_binding: &DependencyStructBinding,
    name: &str,
) -> Option<&'a ql_ast::FunctionDecl> {
    let mut matches = module
        .items
        .iter()
        .flat_map(|item| match &item.kind {
            AstItemKind::Impl(impl_block) => impl_block
                .methods
                .iter()
                .filter(move |method| method.name == name)
                .filter(move |_| {
                    dependency_struct_binding_for_type_expr(package, module, &impl_block.target)
                        .as_ref()
                        .is_some_and(|binding| binding == receiver_binding)
                })
                .collect::<Vec<_>>(),
            AstItemKind::Extend(extend_block) => extend_block
                .methods
                .iter()
                .filter(move |method| method.name == name)
                .filter(move |_| {
                    dependency_struct_binding_for_type_expr(package, module, &extend_block.target)
                        .as_ref()
                        .is_some_and(|binding| binding == receiver_binding)
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return None;
    }
    matches.pop()
}

fn dependency_struct_binding_for_name(
    scopes: &[HashMap<String, DependencyStructBinding>],
    name: &str,
) -> Option<DependencyStructBinding> {
    scopes
        .iter()
        .rev()
        .find_map(|scope| scope.get(name).cloned())
}

fn dependency_struct_element_binding_for_name(
    scopes: &DependencyIterableScopes,
    name: &str,
) -> Option<DependencyStructBinding> {
    for scope in scopes.iter().rev() {
        if let Some(binding) = scope.get(name) {
            return binding.clone();
        }
    }
    None
}

fn dependency_struct_binding_for_definition_target(
    package: &PackageAnalysis,
    target: &DependencyDefinitionTarget,
) -> Option<DependencyStructBinding> {
    if target.kind != SymbolKind::Struct {
        return None;
    }
    let dependency = package
        .dependencies
        .iter()
        .find(|dependency| dependency.interface_path == target.path)?;
    let symbol = dependency.symbols.iter().find(|symbol| {
        symbol.kind == SymbolKind::Struct
            && symbol.source_path == target.source_path
            && symbol.name == target.name
    })?;
    dependency_struct_binding_for_symbol(dependency, symbol)
}

fn dependency_struct_binding_for_call_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    callee: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    match &callee.kind {
        ql_ast::ExprKind::Name(name) => {
            if let Some(function) = local_function_decl_for_name(module, name) {
                let return_type = function.return_type.as_ref()?;
                dependency_struct_binding_for_type_expr(package, module, return_type)
            } else {
                dependency_function_return_binding_for_local_name(package, module, name)
            }
        }
        ql_ast::ExprKind::Member { object, field, .. } => {
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            if let Some(method) = binding.methods.get(field) {
                let return_type = method.return_type_definition.as_ref()?;
                return dependency_struct_binding_for_definition_target(package, return_type);
            }
            let method = local_receiver_method_decl_for_name(package, module, &binding, field)?;
            let return_type = method.return_type.as_ref()?;
            dependency_struct_binding_for_type_expr(package, module, return_type)
        }
        _ => None,
    }
}

fn dependency_struct_binding_for_member_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    object: &ql_ast::Expr,
    field: &str,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
    let field = binding.fields.get(field)?;
    let type_definition = field.type_definition.as_ref()?;
    dependency_struct_binding_for_definition_target(package, type_definition)
}

fn dependency_struct_binding_for_question_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    inner: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    if let Some(binding) = dependency_struct_binding_for_expr(package, module, inner, scopes) {
        return Some(binding);
    }

    match &inner.kind {
        ql_ast::ExprKind::Name(name) => {
            dependency_global_question_binding_for_local_name(package, module, name)
        }
        ql_ast::ExprKind::Member { object, field, .. } => {
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            let field = binding.fields.get(field)?;
            let type_definition = field.question_type_definition.as_ref()?;
            dependency_struct_binding_for_definition_target(package, type_definition)
        }
        ql_ast::ExprKind::Call { callee, .. } => match &callee.kind {
            ql_ast::ExprKind::Name(name) => {
                if let Some(function) = local_function_decl_for_name(module, name) {
                    let return_type = function.return_type.as_ref()?;
                    dependency_struct_question_binding_for_type_expr(package, module, return_type)
                } else {
                    dependency_function_question_return_binding_for_local_name(
                        package, module, name,
                    )
                    .or_else(|| {
                        dependency_global_question_binding_for_local_name(package, module, name)
                    })
                }
            }
            ql_ast::ExprKind::Member { object, field, .. } => {
                let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
                if let Some(method) = binding.methods.get(field) {
                    let return_type = method.question_return_type_definition.as_ref()?;
                    return dependency_struct_binding_for_definition_target(package, return_type);
                }
                let method = local_receiver_method_decl_for_name(package, module, &binding, field)?;
                let return_type = method.return_type.as_ref()?;
                dependency_struct_question_binding_for_type_expr(package, module, return_type)
            }
            _ => None,
        },
        _ => None,
    }
}

fn dependency_struct_binding_for_resolved_field(
    package: &PackageAnalysis,
    field: &DependencyStructResolvedField,
) -> Option<DependencyStructBinding> {
    let type_definition = field.type_definition.as_ref()?;
    dependency_struct_binding_for_definition_target(package, type_definition)
}

fn dependency_struct_binding_for_block_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let mut scopes = scopes.to_vec();
    scopes.push(HashMap::new());
    let mut iterable_scopes = DependencyIterableScopes::new();
    iterable_scopes.push(HashMap::new());
    for stmt in &block.statements {
        if let ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } = &stmt.kind
        {
            bind_dependency_struct_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                &mut scopes,
                &iterable_scopes,
            );
            bind_dependency_iterable_let(
                package,
                module,
                pattern,
                value,
                &scopes,
                &mut iterable_scopes,
            );
        }
    }
    block
        .tail
        .as_ref()
        .and_then(|tail| dependency_struct_binding_for_expr(package, module, tail, &scopes))
}

fn dependency_struct_binding_for_if_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    then_branch: &ql_ast::Block,
    else_branch: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let then_binding =
        dependency_struct_binding_for_block_expr(package, module, then_branch, scopes)?;
    let else_binding = dependency_struct_binding_for_expr(package, module, else_branch, scopes)?;
    (then_binding == else_binding).then_some(then_binding)
}

fn dependency_struct_binding_for_match_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    value: &ql_ast::Expr,
    arms: &[ql_ast::MatchArm],
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
    let mut resolved = None;
    for arm in arms {
        let mut arm_scopes = scopes.to_vec();
        arm_scopes.push(HashMap::new());
        if let Some(binding) = &value_binding {
            bind_dependency_struct_match_pattern(package, &arm.pattern, binding, &mut arm_scopes);
        }
        let body_binding =
            dependency_struct_binding_for_expr(package, module, &arm.body, &arm_scopes)?;
        if resolved
            .as_ref()
            .is_some_and(|binding| binding != &body_binding)
        {
            return None;
        }
        resolved = Some(body_binding);
    }
    resolved
}

fn dependency_struct_common_binding_for_exprs(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    items: &[ql_ast::Expr],
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let mut items = items.iter();
    let first = dependency_struct_binding_for_expr(package, module, items.next()?, scopes)?;
    for item in items {
        let binding = dependency_struct_binding_for_expr(package, module, item, scopes)?;
        if binding != first {
            return None;
        }
    }
    Some(first)
}

fn dependency_struct_element_binding_for_block_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let mut scopes = scopes.to_vec();
    scopes.push(HashMap::new());
    let mut iterable_scopes = iterable_scopes.to_vec();
    iterable_scopes.push(HashMap::new());
    for stmt in &block.statements {
        if let ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } = &stmt.kind
        {
            bind_dependency_struct_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                &mut scopes,
                &iterable_scopes,
            );
            bind_dependency_iterable_let(
                package,
                module,
                pattern,
                value,
                &scopes,
                &mut iterable_scopes,
            );
        }
    }
    block.tail.as_ref().and_then(|tail| {
        dependency_struct_element_binding_for_iterable_expr(
            package,
            module,
            tail,
            &scopes,
            &iterable_scopes,
        )
    })
}

fn dependency_struct_question_element_binding_for_block_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let mut scopes = scopes.to_vec();
    scopes.push(HashMap::new());
    let mut iterable_scopes = iterable_scopes.to_vec();
    iterable_scopes.push(HashMap::new());
    for stmt in &block.statements {
        if let ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } = &stmt.kind
        {
            bind_dependency_struct_let(
                package,
                module,
                pattern,
                ty.as_ref(),
                value,
                &mut scopes,
                &iterable_scopes,
            );
            bind_dependency_iterable_let(
                package,
                module,
                pattern,
                value,
                &scopes,
                &mut iterable_scopes,
            );
        }
    }
    block.tail.as_ref().and_then(|tail| {
        dependency_struct_element_binding_for_question_expr(
            package,
            module,
            tail,
            &scopes,
            &iterable_scopes,
        )
    })
}

fn dependency_struct_element_binding_for_if_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    then_branch: &ql_ast::Block,
    else_branch: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let then_binding = dependency_struct_element_binding_for_block_expr(
        package,
        module,
        then_branch,
        scopes,
        iterable_scopes,
    )?;
    let else_binding = dependency_struct_element_binding_for_iterable_expr(
        package,
        module,
        else_branch,
        scopes,
        iterable_scopes,
    )?;
    (then_binding == else_binding).then_some(then_binding)
}

fn dependency_struct_question_element_binding_for_if_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    then_branch: &ql_ast::Block,
    else_branch: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let then_binding = dependency_struct_question_element_binding_for_block_expr(
        package,
        module,
        then_branch,
        scopes,
        iterable_scopes,
    )?;
    let else_binding = dependency_struct_element_binding_for_question_expr(
        package,
        module,
        else_branch,
        scopes,
        iterable_scopes,
    )?;
    (then_binding == else_binding).then_some(then_binding)
}

fn dependency_struct_element_binding_for_match_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    value: &ql_ast::Expr,
    arms: &[ql_ast::MatchArm],
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
    let mut resolved = None;
    for arm in arms {
        let mut arm_scopes = scopes.to_vec();
        arm_scopes.push(HashMap::new());
        let mut arm_iterable_scopes = iterable_scopes.to_vec();
        arm_iterable_scopes.push(HashMap::new());
        if let Some(binding) = &value_binding {
            bind_dependency_struct_match_pattern(package, &arm.pattern, binding, &mut arm_scopes);
        }
        shadow_dependency_iterable_pattern(&arm.pattern, &mut arm_iterable_scopes);
        let body_binding = dependency_struct_element_binding_for_iterable_expr(
            package,
            module,
            &arm.body,
            &arm_scopes,
            &arm_iterable_scopes,
        )?;
        if resolved
            .as_ref()
            .is_some_and(|binding| binding != &body_binding)
        {
            return None;
        }
        resolved = Some(body_binding);
    }
    resolved
}

fn dependency_struct_question_element_binding_for_match_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    value: &ql_ast::Expr,
    arms: &[ql_ast::MatchArm],
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
    let mut resolved = None;
    for arm in arms {
        let mut arm_scopes = scopes.to_vec();
        arm_scopes.push(HashMap::new());
        let mut arm_iterable_scopes = iterable_scopes.to_vec();
        arm_iterable_scopes.push(HashMap::new());
        if let Some(binding) = &value_binding {
            bind_dependency_struct_match_pattern(package, &arm.pattern, binding, &mut arm_scopes);
        }
        shadow_dependency_iterable_pattern(&arm.pattern, &mut arm_iterable_scopes);
        let body_binding = dependency_struct_element_binding_for_question_expr(
            package,
            module,
            &arm.body,
            &arm_scopes,
            &arm_iterable_scopes,
        )?;
        if resolved
            .as_ref()
            .is_some_and(|binding| binding != &body_binding)
        {
            return None;
        }
        resolved = Some(body_binding);
    }
    resolved
}

fn dependency_struct_element_binding_for_call_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    callee: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    match &callee.kind {
        ql_ast::ExprKind::Name(name) => {
            if let Some(function) = local_function_decl_for_name(module, name) {
                let return_type = function.return_type.as_ref()?;
                dependency_struct_element_binding_for_type_expr(package, module, return_type)
            } else {
                dependency_function_iterable_element_binding_for_local_name(package, module, name)
                    .or_else(|| {
                        dependency_global_iterable_element_binding_for_local_name(
                            package, module, name,
                        )
                    })
            }
        }
        ql_ast::ExprKind::Member { object, field, .. } => {
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            if let Some(method) = binding.methods.get(field) {
                let target = method.iterable_element_type_definition.as_ref()?;
                return dependency_struct_binding_for_definition_target(package, target);
            }
            let method = local_receiver_method_decl_for_name(package, module, &binding, field)?;
            let return_type = method.return_type.as_ref()?;
            dependency_struct_element_binding_for_type_expr(package, module, return_type)
        }
        _ => None,
    }
}

fn dependency_struct_element_binding_for_member_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    object: &ql_ast::Expr,
    field: &str,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
    let field = binding.fields.get(field)?;
    let target = field.iterable_element_type_definition.as_ref()?;
    dependency_struct_binding_for_definition_target(package, target)
}

fn dependency_struct_element_binding_for_question_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    inner: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    if let Some(binding) = dependency_struct_element_binding_for_iterable_expr(
        package,
        module,
        inner,
        scopes,
        iterable_scopes,
    ) {
        return Some(binding);
    }

    match &inner.kind {
        ql_ast::ExprKind::Name(name) => {
            dependency_global_question_iterable_element_binding_for_local_name(
                package, module, name,
            )
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_struct_question_element_binding_for_block_expr(
                package,
                module,
                block,
                scopes,
                iterable_scopes,
            )
        }
        ql_ast::ExprKind::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => dependency_struct_question_element_binding_for_if_expr(
            package,
            module,
            then_branch,
            else_branch,
            scopes,
            iterable_scopes,
        ),
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_struct_question_element_binding_for_match_expr(
                package,
                module,
                value,
                arms,
                scopes,
                iterable_scopes,
            )
        }
        ql_ast::ExprKind::Member { object, field, .. } => {
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            let field = binding.fields.get(field)?;
            let target = field.question_iterable_element_type_definition.as_ref()?;
            dependency_struct_binding_for_definition_target(package, target)
        }
        ql_ast::ExprKind::Call { callee, .. } => match &callee.kind {
            ql_ast::ExprKind::Name(name) => {
                if let Some(function) = local_function_decl_for_name(module, name) {
                    let return_type = function.return_type.as_ref()?;
                    dependency_struct_question_element_binding_for_type_expr(
                        package,
                        module,
                        return_type,
                    )
                } else {
                    dependency_function_question_iterable_element_binding_for_local_name(
                        package, module, name,
                    )
                    .or_else(|| {
                        dependency_global_question_iterable_element_binding_for_local_name(
                            package, module, name,
                        )
                    })
                }
            }
            ql_ast::ExprKind::Member { object, field, .. } => {
                let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
                if let Some(method) = binding.methods.get(field) {
                    let target = method.question_iterable_element_type_definition.as_ref()?;
                    return dependency_struct_binding_for_definition_target(package, target);
                }
                let method = local_receiver_method_decl_for_name(package, module, &binding, field)?;
                let return_type = method.return_type.as_ref()?;
                dependency_struct_question_element_binding_for_type_expr(
                    package,
                    module,
                    return_type,
                )
            }
            _ => None,
        },
        _ => None,
    }
}

fn dependency_struct_element_binding_for_iterable_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Name(name) => {
            dependency_struct_element_binding_for_name(iterable_scopes, name).or_else(|| {
                dependency_global_iterable_element_binding_for_local_name(package, module, name)
            })
        }
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            dependency_struct_common_binding_for_exprs(package, module, items, scopes)
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_struct_element_binding_for_block_expr(
                package,
                module,
                block,
                scopes,
                iterable_scopes,
            )
        }
        ql_ast::ExprKind::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => dependency_struct_element_binding_for_if_expr(
            package,
            module,
            then_branch,
            else_branch,
            scopes,
            iterable_scopes,
        ),
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_struct_element_binding_for_match_expr(
                package,
                module,
                value,
                arms,
                scopes,
                iterable_scopes,
            )
        }
        ql_ast::ExprKind::Call { callee, .. } => {
            dependency_struct_element_binding_for_call_expr(package, module, callee, scopes)
        }
        ql_ast::ExprKind::Member { object, field, .. } => {
            dependency_struct_element_binding_for_member_expr(
                package, module, object, field, scopes,
            )
        }
        ql_ast::ExprKind::Question(inner) => dependency_struct_element_binding_for_question_expr(
            package,
            module,
            inner,
            scopes,
            iterable_scopes,
        ),
        _ => None,
    }
}

fn dependency_struct_binding_for_bracket_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    target: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    let iterable_scopes = DependencyIterableScopes::new();
    dependency_struct_element_binding_for_iterable_expr(
        package,
        module,
        target,
        scopes,
        &iterable_scopes,
    )
}

fn dependency_struct_binding_for_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Name(name) => dependency_struct_binding_for_name(scopes, name)
            .or_else(|| dependency_global_binding_for_local_name(package, module, name)),
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            dependency_struct_common_binding_for_exprs(package, module, items, scopes)
        }
        ql_ast::ExprKind::StructLiteral { path, .. } => {
            let [root_name] = path.segments.as_slice() else {
                return None;
            };
            dependency_struct_binding_for_local_name(package, module, root_name)
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_struct_binding_for_block_expr(package, module, block, scopes)
        }
        ql_ast::ExprKind::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => {
            dependency_struct_binding_for_if_expr(package, module, then_branch, else_branch, scopes)
        }
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_struct_binding_for_match_expr(package, module, value, arms, scopes)
        }
        ql_ast::ExprKind::Member { object, field, .. } => {
            dependency_struct_binding_for_member_expr(package, module, object, field, scopes)
        }
        ql_ast::ExprKind::Call { callee, .. } => {
            dependency_struct_binding_for_call_expr(package, module, callee, scopes)
        }
        ql_ast::ExprKind::Bracket { target, .. } => {
            dependency_struct_binding_for_bracket_expr(package, module, target, scopes)
        }
        ql_ast::ExprKind::Question(inner) => {
            dependency_struct_binding_for_question_expr(package, module, inner, scopes)
        }
        _ => None,
    }
}

fn bind_dependency_struct_param(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    param: &ql_ast::Param,
    receiver_binding: Option<&DependencyStructBinding>,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
) {
    let scope = scopes.last_mut().expect("scope stack must be non-empty");
    match param {
        ql_ast::Param::Regular { name, ty, .. } => {
            let Some(binding) = dependency_struct_binding_for_type_expr(package, module, ty) else {
                return;
            };
            scope.insert(name.clone(), binding);
        }
        ql_ast::Param::Receiver { .. } => {
            let Some(binding) = receiver_binding else {
                return;
            };
            scope.insert(String::from("self"), binding.clone());
        }
    }
}

fn bind_dependency_struct_pattern(
    package: &PackageAnalysis,
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
) {
    match &pattern.kind {
        ql_ast::PatternKind::Name(name) => {
            scopes
                .last_mut()
                .expect("scope stack must be non-empty")
                .insert(name.clone(), binding.clone());
        }
        ql_ast::PatternKind::Tuple(items) | ql_ast::PatternKind::Array(items) => {
            for item in items {
                bind_dependency_struct_pattern(package, item, binding, scopes);
            }
        }
        ql_ast::PatternKind::Struct { fields, .. } => {
            for field in fields {
                let Some(field_binding) = binding
                    .fields
                    .get(&field.name)
                    .and_then(|field| dependency_struct_binding_for_resolved_field(package, field))
                else {
                    continue;
                };
                if let Some(pattern) = &field.pattern {
                    bind_dependency_struct_pattern(package, pattern, &field_binding, scopes);
                } else {
                    scopes
                        .last_mut()
                        .expect("scope stack must be non-empty")
                        .insert(field.name.clone(), field_binding);
                }
            }
        }
        _ => {}
    }
}

fn bind_dependency_struct_closure_param(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    param: &ql_ast::ClosureParam,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
) {
    let Some(ty) = &param.ty else {
        return;
    };
    let Some(binding) = dependency_struct_binding_for_type_expr(package, module, ty) else {
        return;
    };
    scopes
        .last_mut()
        .expect("scope stack must be non-empty")
        .insert(param.name.clone(), binding);
}

fn shadow_dependency_iterable_pattern(
    pattern: &ql_ast::Pattern,
    scopes: &mut DependencyIterableScopes,
) {
    match &pattern.kind {
        ql_ast::PatternKind::Name(name) => {
            scopes
                .last_mut()
                .expect("iterable scope stack must be non-empty")
                .insert(name.clone(), None);
        }
        ql_ast::PatternKind::Tuple(items)
        | ql_ast::PatternKind::Array(items)
        | ql_ast::PatternKind::TupleStruct { items, .. } => {
            for item in items {
                shadow_dependency_iterable_pattern(item, scopes);
            }
        }
        ql_ast::PatternKind::Struct { fields, .. } => {
            for field in fields {
                if let Some(pattern) = &field.pattern {
                    shadow_dependency_iterable_pattern(pattern, scopes);
                } else {
                    scopes
                        .last_mut()
                        .expect("iterable scope stack must be non-empty")
                        .insert(field.name.clone(), None);
                }
            }
        }
        ql_ast::PatternKind::Path(_)
        | ql_ast::PatternKind::Integer(_)
        | ql_ast::PatternKind::String(_)
        | ql_ast::PatternKind::Bool(_)
        | ql_ast::PatternKind::NoneLiteral
        | ql_ast::PatternKind::Wildcard => {}
    }
}

fn bind_dependency_iterable_alias_pattern(
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    scopes: &mut DependencyIterableScopes,
) {
    match &pattern.kind {
        ql_ast::PatternKind::Name(name) => {
            scopes
                .last_mut()
                .expect("iterable scope stack must be non-empty")
                .insert(name.clone(), Some(binding.clone()));
        }
        _ => shadow_dependency_iterable_pattern(pattern, scopes),
    }
}

fn bind_dependency_iterable_param(param: &ql_ast::Param, scopes: &mut DependencyIterableScopes) {
    let name = match param {
        ql_ast::Param::Regular { name, .. } => name,
        ql_ast::Param::Receiver { .. } => "self",
    };
    scopes
        .last_mut()
        .expect("iterable scope stack must be non-empty")
        .insert(String::from(name), None);
}

fn bind_dependency_iterable_closure_param(
    param: &ql_ast::ClosureParam,
    scopes: &mut DependencyIterableScopes,
) {
    scopes
        .last_mut()
        .expect("iterable scope stack must be non-empty")
        .insert(param.name.clone(), None);
}

fn pattern_destructures_dependency_iterable_elements(pattern: &ql_ast::Pattern) -> bool {
    matches!(
        pattern.kind,
        ql_ast::PatternKind::Tuple(_) | ql_ast::PatternKind::Array(_)
    )
}

fn bind_dependency_struct_let(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    pattern: &ql_ast::Pattern,
    ty: Option<&ql_ast::TypeExpr>,
    value: &ql_ast::Expr,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
) {
    let binding = ty
        .and_then(|ty| dependency_struct_binding_for_type_expr(package, module, ty))
        .or_else(|| dependency_struct_binding_for_expr(package, module, value, scopes))
        .or_else(|| {
            if pattern_destructures_dependency_iterable_elements(pattern) {
                dependency_struct_element_binding_for_iterable_expr(
                    package,
                    module,
                    value,
                    scopes,
                    iterable_scopes,
                )
            } else {
                None
            }
        });
    let Some(binding) = binding else {
        return;
    };
    bind_dependency_struct_pattern(package, pattern, &binding, scopes);
}

fn bind_dependency_iterable_let(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    pattern: &ql_ast::Pattern,
    value: &ql_ast::Expr,
    binding_scopes: &[HashMap<String, DependencyStructBinding>],
    iterable_scopes: &mut DependencyIterableScopes,
) {
    let binding = dependency_struct_element_binding_for_iterable_expr(
        package,
        module,
        value,
        binding_scopes,
        iterable_scopes,
    );
    if let Some(binding) = &binding {
        bind_dependency_iterable_alias_pattern(pattern, binding, iterable_scopes);
    } else {
        shadow_dependency_iterable_pattern(pattern, iterable_scopes);
    }
}

fn bind_dependency_struct_match_pattern(
    package: &PackageAnalysis,
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
) {
    bind_dependency_struct_pattern(package, pattern, binding, scopes);
}

fn dependency_value_binding_for_name<'a>(
    scopes: &'a [HashMap<String, DependencyValueBinding>],
    name: &str,
) -> Option<&'a DependencyValueBinding> {
    scopes.iter().rev().find_map(|scope| scope.get(name))
}

fn push_dependency_value_occurrence(
    binding: &DependencyValueBinding,
    reference_span: Span,
    is_definition: bool,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    occurrences.push(DependencyValueOccurrence {
        kind: binding.kind,
        local_name: binding.local_name.clone(),
        reference_span,
        definition_span: binding.definition_span,
        definition_rename: binding.definition_rename.clone(),
        package_name: binding.dependency.package_name.clone(),
        manifest_path: binding.dependency.manifest_path.clone(),
        source_path: binding.dependency.source_path.clone(),
        struct_name: binding.dependency.struct_name.clone(),
        path: binding.dependency.path.clone(),
        is_definition,
    });
}

fn push_dependency_value_root_occurrence(
    kind: SymbolKind,
    local_name: &str,
    reference_span: Span,
    dependency: &DependencyStructBinding,
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    let binding = DependencyValueBinding {
        kind,
        local_name: local_name.to_owned(),
        definition_span: dependency.definition_span,
        definition_rename: DependencyValueDefinitionRename::Direct,
        dependency: dependency.clone(),
    };
    push_dependency_value_occurrence(&binding, reference_span, false, occurrences);
}

fn push_dependency_indexed_value_root_occurrence_for_bracket_target(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    target: &ql_ast::Expr,
    binding_scopes: &[HashMap<String, DependencyStructBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    let Some(binding) =
        dependency_struct_binding_for_bracket_expr(package, module, target, binding_scopes)
    else {
        return;
    };

    match &target.kind {
        ql_ast::ExprKind::Member {
            field, field_span, ..
        } => {
            push_dependency_value_root_occurrence(
                SymbolKind::Field,
                field,
                *field_span,
                &binding,
                occurrences,
            );
        }
        ql_ast::ExprKind::Call { callee, .. } => {
            if let ql_ast::ExprKind::Member {
                field, field_span, ..
            } = &callee.kind
            {
                push_dependency_value_root_occurrence(
                    SymbolKind::Method,
                    field,
                    *field_span,
                    &binding,
                    occurrences,
                );
            }
        }
        ql_ast::ExprKind::Question(inner) => match &inner.kind {
            ql_ast::ExprKind::Member {
                field, field_span, ..
            } => {
                push_dependency_value_root_occurrence(
                    SymbolKind::Field,
                    field,
                    *field_span,
                    &binding,
                    occurrences,
                );
            }
            ql_ast::ExprKind::Call { callee, .. } => {
                if let ql_ast::ExprKind::Member {
                    field, field_span, ..
                } = &callee.kind
                {
                    push_dependency_value_root_occurrence(
                        SymbolKind::Method,
                        field,
                        *field_span,
                        &binding,
                        occurrences,
                    );
                }
            }
            _ => {}
        },
        ql_ast::ExprKind::Bracket { target, .. } => {
            push_dependency_indexed_value_root_occurrence_for_bracket_target(
                package,
                module,
                target,
                binding_scopes,
                occurrences,
            );
        }
        _ => {}
    }
}

fn bind_dependency_value_local(
    kind: SymbolKind,
    local_name: String,
    definition_span: Span,
    definition_rename: DependencyValueDefinitionRename,
    dependency: DependencyStructBinding,
    binding_scopes: &mut [HashMap<String, DependencyStructBinding>],
    value_scopes: &mut [HashMap<String, DependencyValueBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    binding_scopes
        .last_mut()
        .expect("scope stack must be non-empty")
        .insert(local_name.clone(), dependency.clone());
    let binding = DependencyValueBinding {
        kind,
        local_name,
        definition_span,
        definition_rename,
        dependency,
    };
    value_scopes
        .last_mut()
        .expect("scope stack must be non-empty")
        .insert(binding.local_name.clone(), binding.clone());
    push_dependency_value_occurrence(&binding, binding.definition_span, true, occurrences);
}

fn bind_dependency_value_param(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    param: &ql_ast::Param,
    receiver_binding: Option<&DependencyStructBinding>,
    binding_scopes: &mut [HashMap<String, DependencyStructBinding>],
    value_scopes: &mut [HashMap<String, DependencyValueBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    match param {
        ql_ast::Param::Regular {
            name,
            name_span,
            ty,
        } => {
            let Some(binding) = dependency_struct_binding_for_type_expr(package, module, ty) else {
                return;
            };
            bind_dependency_value_local(
                SymbolKind::Parameter,
                name.clone(),
                *name_span,
                DependencyValueDefinitionRename::Direct,
                binding,
                binding_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::Param::Receiver { span, .. } => {
            let Some(binding) = receiver_binding.cloned() else {
                return;
            };
            bind_dependency_value_local(
                SymbolKind::SelfParameter,
                String::from("self"),
                *span,
                DependencyValueDefinitionRename::Direct,
                binding,
                binding_scopes,
                value_scopes,
                occurrences,
            );
        }
    }
}

fn bind_dependency_value_closure_param(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    param: &ql_ast::ClosureParam,
    binding_scopes: &mut [HashMap<String, DependencyStructBinding>],
    value_scopes: &mut [HashMap<String, DependencyValueBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    let Some(ty) = &param.ty else {
        return;
    };
    let Some(binding) = dependency_struct_binding_for_type_expr(package, module, ty) else {
        return;
    };
    bind_dependency_value_local(
        SymbolKind::Parameter,
        param.name.clone(),
        param.span,
        DependencyValueDefinitionRename::Direct,
        binding,
        binding_scopes,
        value_scopes,
        occurrences,
    );
}

fn bind_dependency_value_pattern(
    package: &PackageAnalysis,
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    binding_scopes: &mut [HashMap<String, DependencyStructBinding>],
    value_scopes: &mut [HashMap<String, DependencyValueBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    match &pattern.kind {
        ql_ast::PatternKind::Name(name) => {
            bind_dependency_value_local(
                SymbolKind::Local,
                name.clone(),
                pattern.span,
                DependencyValueDefinitionRename::Direct,
                binding.clone(),
                binding_scopes,
                value_scopes,
                occurrences,
            );
        }
        ql_ast::PatternKind::Tuple(items) | ql_ast::PatternKind::Array(items) => {
            for item in items {
                bind_dependency_value_pattern(
                    package,
                    item,
                    binding,
                    binding_scopes,
                    value_scopes,
                    occurrences,
                );
            }
        }
        ql_ast::PatternKind::Struct { fields, .. } => {
            for field in fields {
                let Some(field_binding) = binding
                    .fields
                    .get(&field.name)
                    .and_then(|field| dependency_struct_binding_for_resolved_field(package, field))
                else {
                    continue;
                };
                if let Some(pattern) = &field.pattern {
                    bind_dependency_value_pattern(
                        package,
                        pattern,
                        &field_binding,
                        binding_scopes,
                        value_scopes,
                        occurrences,
                    );
                } else {
                    bind_dependency_value_local(
                        SymbolKind::Local,
                        field.name.clone(),
                        field.name_span,
                        DependencyValueDefinitionRename::StructShorthandField {
                            field_name: field.name.clone(),
                        },
                        field_binding,
                        binding_scopes,
                        value_scopes,
                        occurrences,
                    );
                }
            }
        }
        _ => {}
    }
}

fn bind_dependency_value_let(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    pattern: &ql_ast::Pattern,
    ty: Option<&ql_ast::TypeExpr>,
    value: &ql_ast::Expr,
    binding_scopes: &mut [HashMap<String, DependencyStructBinding>],
    iterable_scopes: &DependencyIterableScopes,
    value_scopes: &mut [HashMap<String, DependencyValueBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    let binding = ty
        .and_then(|ty| dependency_struct_binding_for_type_expr(package, module, ty))
        .or_else(|| dependency_struct_binding_for_expr(package, module, value, binding_scopes))
        .or_else(|| {
            if pattern_destructures_dependency_iterable_elements(pattern) {
                dependency_struct_element_binding_for_iterable_expr(
                    package,
                    module,
                    value,
                    binding_scopes,
                    iterable_scopes,
                )
            } else {
                None
            }
        });
    let Some(binding) = binding else {
        return;
    };
    bind_dependency_value_pattern(
        package,
        pattern,
        &binding,
        binding_scopes,
        value_scopes,
        occurrences,
    );
}

fn bind_dependency_value_match_pattern(
    package: &PackageAnalysis,
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    binding_scopes: &mut [HashMap<String, DependencyStructBinding>],
    value_scopes: &mut [HashMap<String, DependencyValueBinding>],
    occurrences: &mut Vec<DependencyValueOccurrence>,
) {
    bind_dependency_value_pattern(
        package,
        pattern,
        binding,
        binding_scopes,
        value_scopes,
        occurrences,
    );
}

fn push_dependency_struct_field_occurrence_for_binding(
    binding: &DependencyStructBinding,
    field_name: &str,
    field_span: Span,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    let Some(field) = binding.fields.get(field_name) else {
        return;
    };
    occurrences.push(DependencyStructFieldOccurrence {
        reference_span: field_span,
        package_name: binding.package_name.clone(),
        manifest_path: binding.manifest_path.clone(),
        source_path: binding.source_path.clone(),
        struct_name: binding.struct_name.clone(),
        name: field.name.clone(),
        detail: field.detail.clone(),
        path: binding.path.clone(),
        definition_span: field.definition_span,
    });
}

fn push_dependency_method_occurrence_for_binding(
    binding: &DependencyStructBinding,
    method_name: &str,
    method_span: Span,
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    let Some(method) = binding.methods.get(method_name) else {
        return;
    };
    occurrences.push(DependencyMethodOccurrence {
        reference_span: method_span,
        package_name: binding.package_name.clone(),
        manifest_path: binding.manifest_path.clone(),
        source_path: method.source_path.clone(),
        struct_name: binding.struct_name.clone(),
        name: method.name.clone(),
        detail: method.detail.clone(),
        path: binding.path.clone(),
        definition_span: method.definition_span,
    });
}

fn dependency_type_expr_targets_struct(ty: &ql_ast::TypeExpr, struct_name: &str) -> bool {
    let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| segment == struct_name)
}

fn dependency_import_binding_for_local_name<'a>(
    package: &'a PackageAnalysis,
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<(&'a DependencyInterface, &'a DependencySymbol)> {
    let mut matches = module
        .uses
        .iter()
        .flat_map(|use_decl| dependency_import_bindings_for_local_name(use_decl, local_name))
        .filter_map(|binding| package.resolve_dependency_import_binding(&binding))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return None;
    }
    matches.pop()
}

fn dependency_unique_import_binding_for_local_name(
    module: &ql_ast::Module,
    local_name: &str,
) -> Option<ImportBinding> {
    let mut matches = module
        .uses
        .iter()
        .flat_map(|use_decl| dependency_import_bindings_for_local_name(use_decl, local_name))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return None;
    }
    matches.pop()
}

fn dependency_import_bindings_for_local_name(
    use_decl: &ql_ast::UseDecl,
    local_name: &str,
) -> Vec<ImportBinding> {
    if let Some(group) = &use_decl.group {
        group
            .iter()
            .filter_map(|item| {
                let binding = ImportBinding::grouped(&use_decl.prefix, item);
                (binding.local_name == local_name).then_some(binding)
            })
            .collect()
    } else {
        let binding = ImportBinding::direct(use_decl);
        if binding.local_name == local_name {
            vec![binding]
        } else {
            Vec::new()
        }
    }
}

fn dependency_import_binding_imported_name(binding: &ImportBinding) -> Option<&str> {
    binding.path.segments.last().map(String::as_str)
}

fn dependency_import_binding_uses_direct_local_name(binding: &ImportBinding) -> bool {
    dependency_import_binding_imported_name(binding)
        .is_some_and(|imported_name| binding.local_name == imported_name)
}

fn dependency_value_occurrence_supports_same_file_rename(
    occurrence: &DependencyValueOccurrence,
) -> bool {
    matches!(occurrence.kind, SymbolKind::Local | SymbolKind::Parameter)
}

fn dependency_value_occurrence_matches_rename_target(
    occurrence: &DependencyValueOccurrence,
    target: &DependencyValueOccurrence,
) -> bool {
    occurrence.kind == target.kind
        && occurrence.local_name == target.local_name
        && occurrence.definition_span == target.definition_span
        && occurrence.package_name == target.package_name
        && occurrence.source_path == target.source_path
        && occurrence.struct_name == target.struct_name
        && occurrence.path == target.path
}

fn dependency_value_occurrence_rename_replacement(
    occurrence: &DependencyValueOccurrence,
    new_name: &str,
) -> String {
    if occurrence.is_definition {
        if let DependencyValueDefinitionRename::StructShorthandField { field_name } =
            &occurrence.definition_rename
        {
            return format!("{field_name}: {new_name}");
        }
    }

    new_name.to_owned()
}

fn validate_dependency_rename_text(text: &str) -> Result<(), RenameError> {
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

fn dependency_value_root_binding_in_module(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    offset: usize,
) -> Option<DependencyStructBinding> {
    module
        .items
        .iter()
        .find_map(|item| dependency_value_root_binding_in_item(package, module, item, offset))
}

fn dependency_value_root_binding_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    offset: usize,
) -> Option<DependencyStructBinding> {
    match &item.kind {
        ql_ast::ItemKind::Function(function) => function
            .body
            .as_ref()
            .and_then(|body| dependency_value_root_binding_in_block(package, module, body, offset)),
        ql_ast::ItemKind::Const(global) | ql_ast::ItemKind::Static(global) => {
            dependency_value_root_binding_in_expr(package, module, &global.value, offset)
        }
        ql_ast::ItemKind::Impl(impl_block) => impl_block.methods.iter().find_map(|method| {
            method.body.as_ref().and_then(|body| {
                dependency_value_root_binding_in_block(package, module, body, offset)
            })
        }),
        ql_ast::ItemKind::Extend(extend_block) => extend_block.methods.iter().find_map(|method| {
            method.body.as_ref().and_then(|body| {
                dependency_value_root_binding_in_block(package, module, body, offset)
            })
        }),
        _ => None,
    }
}

fn dependency_value_root_binding_in_block(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    offset: usize,
) -> Option<DependencyStructBinding> {
    block
        .statements
        .iter()
        .find_map(|stmt| dependency_value_root_binding_in_stmt(package, module, stmt, offset))
        .or_else(|| {
            block.tail.as_ref().and_then(|expr| {
                dependency_value_root_binding_in_expr(package, module, expr, offset)
            })
        })
}

fn dependency_value_root_binding_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    offset: usize,
) -> Option<DependencyStructBinding> {
    match &stmt.kind {
        ql_ast::StmtKind::Let { value, .. }
        | ql_ast::StmtKind::Expr { expr: value, .. }
        | ql_ast::StmtKind::Defer(value)
        | ql_ast::StmtKind::Return(Some(value)) => {
            dependency_value_root_binding_in_expr(package, module, value, offset)
        }
        ql_ast::StmtKind::While { condition, body } => {
            dependency_value_root_binding_in_expr(package, module, condition, offset)
                .or_else(|| dependency_value_root_binding_in_block(package, module, body, offset))
        }
        ql_ast::StmtKind::Loop { body } => {
            dependency_value_root_binding_in_block(package, module, body, offset)
        }
        ql_ast::StmtKind::For { iterable, body, .. } => {
            dependency_value_root_iterable_binding_in_expr(package, module, iterable, offset)
                .or_else(|| {
                    dependency_value_root_binding_in_expr(package, module, iterable, offset)
                })
                .or_else(|| dependency_value_root_binding_in_block(package, module, body, offset))
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn dependency_value_root_iterable_binding_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    offset: usize,
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Name(name)
            if dependency_struct_field_completion_span_contains(expr.span, offset) =>
        {
            dependency_function_iterable_element_binding_for_local_name(package, module, name)
                .or_else(|| {
                    dependency_global_iterable_element_binding_for_local_name(package, module, name)
                })
        }
        ql_ast::ExprKind::Name(_) => None,
        ql_ast::ExprKind::Call { callee, args } => {
            if let ql_ast::ExprKind::Name(name) = &callee.kind
                && dependency_struct_field_completion_span_contains(callee.span, offset)
                && let Some(binding) = dependency_function_iterable_element_binding_for_local_name(
                    package, module, name,
                )
            {
                return Some(binding);
            }

            dependency_value_root_iterable_binding_in_expr(package, module, callee, offset).or_else(
                || {
                    args.iter().find_map(|arg| match arg {
                        ql_ast::CallArg::Positional(expr) => {
                            dependency_value_root_iterable_binding_in_expr(
                                package, module, expr, offset,
                            )
                        }
                        ql_ast::CallArg::Named { value, .. } => {
                            dependency_value_root_iterable_binding_in_expr(
                                package, module, value, offset,
                            )
                        }
                    })
                },
            )
        }
        ql_ast::ExprKind::Question(inner) => match &inner.kind {
            ql_ast::ExprKind::Name(name)
                if dependency_struct_field_completion_span_contains(inner.span, offset) =>
            {
                dependency_function_question_iterable_element_binding_for_local_name(
                    package, module, name,
                )
                .or_else(|| {
                    dependency_global_question_iterable_element_binding_for_local_name(
                        package, module, name,
                    )
                })
            }
            ql_ast::ExprKind::Call { callee, .. }
                if matches!(&callee.kind, ql_ast::ExprKind::Name(_))
                    && dependency_struct_field_completion_span_contains(callee.span, offset) =>
            {
                let ql_ast::ExprKind::Name(name) = &callee.kind else {
                    unreachable!()
                };
                dependency_function_question_iterable_element_binding_for_local_name(
                    package, module, name,
                )
                .or_else(|| {
                    dependency_global_question_iterable_element_binding_for_local_name(
                        package, module, name,
                    )
                })
            }
            _ => dependency_value_root_iterable_binding_in_expr(package, module, inner, offset),
        },
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            items.iter().find_map(|expr| {
                dependency_value_root_iterable_binding_in_expr(package, module, expr, offset)
            })
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_value_root_iterable_binding_in_block(package, module, block, offset)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_value_root_iterable_binding_in_expr(package, module, condition, offset)
            .or_else(|| {
                dependency_value_root_iterable_binding_in_block(
                    package,
                    module,
                    then_branch,
                    offset,
                )
            })
            .or_else(|| {
                else_branch.as_ref().and_then(|expr| {
                    dependency_value_root_iterable_binding_in_expr(package, module, expr, offset)
                })
            }),
        ql_ast::ExprKind::Match { value, arms } => dependency_value_root_iterable_binding_in_expr(
            package, module, value, offset,
        )
        .or_else(|| {
            arms.iter().find_map(|arm| {
                arm.guard
                    .as_ref()
                    .and_then(|expr| {
                        dependency_value_root_iterable_binding_in_expr(
                            package, module, expr, offset,
                        )
                    })
                    .or_else(|| {
                        dependency_value_root_iterable_binding_in_expr(
                            package, module, &arm.body, offset,
                        )
                    })
            })
        }),
        ql_ast::ExprKind::Closure { body, .. } => {
            dependency_value_root_iterable_binding_in_expr(package, module, body, offset)
        }
        ql_ast::ExprKind::Member { object, .. } | ql_ast::ExprKind::Unary { expr: object, .. } => {
            dependency_value_root_iterable_binding_in_expr(package, module, object, offset)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            dependency_value_root_iterable_binding_in_expr(package, module, target, offset).or_else(
                || {
                    items.iter().find_map(|expr| {
                        dependency_value_root_iterable_binding_in_expr(
                            package, module, expr, offset,
                        )
                    })
                },
            )
        }
        ql_ast::ExprKind::StructLiteral { fields, .. } => fields.iter().find_map(|field| {
            field.value.as_ref().and_then(|expr| {
                dependency_value_root_iterable_binding_in_expr(package, module, expr, offset)
            })
        }),
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_value_root_iterable_binding_in_expr(package, module, left, offset).or_else(
                || dependency_value_root_iterable_binding_in_expr(package, module, right, offset),
            )
        }
        ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
    }
}

fn dependency_value_root_iterable_binding_in_block(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    block: &ql_ast::Block,
    offset: usize,
) -> Option<DependencyStructBinding> {
    block
        .statements
        .iter()
        .find_map(|stmt| {
            dependency_value_root_iterable_binding_in_stmt(package, module, stmt, offset)
        })
        .or_else(|| {
            block.tail.as_ref().and_then(|tail| {
                dependency_value_root_iterable_binding_in_expr(package, module, tail, offset)
            })
        })
}

fn dependency_value_root_iterable_binding_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    offset: usize,
) -> Option<DependencyStructBinding> {
    match &stmt.kind {
        ql_ast::StmtKind::Let { value, .. }
        | ql_ast::StmtKind::Expr { expr: value, .. }
        | ql_ast::StmtKind::Defer(value)
        | ql_ast::StmtKind::Return(Some(value)) => {
            dependency_value_root_iterable_binding_in_expr(package, module, value, offset)
        }
        ql_ast::StmtKind::While { condition, body } => {
            dependency_value_root_iterable_binding_in_expr(package, module, condition, offset)
                .or_else(|| {
                    dependency_value_root_iterable_binding_in_block(package, module, body, offset)
                })
        }
        ql_ast::StmtKind::Loop { body } => {
            dependency_value_root_iterable_binding_in_block(package, module, body, offset)
        }
        ql_ast::StmtKind::For { iterable, body, .. } => {
            dependency_value_root_iterable_binding_in_expr(package, module, iterable, offset)
                .or_else(|| {
                    dependency_value_root_iterable_binding_in_block(package, module, body, offset)
                })
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {
            None
        }
    }
}

fn dependency_value_root_binding_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    offset: usize,
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Name(name)
            if dependency_struct_field_completion_span_contains(expr.span, offset) =>
        {
            dependency_global_binding_for_local_name(package, module, name)
        }
        ql_ast::ExprKind::Name(_) => None,
        ql_ast::ExprKind::Call { callee, args } => {
            if let ql_ast::ExprKind::Name(name) = &callee.kind
                && dependency_struct_field_completion_span_contains(callee.span, offset)
                && let Some(binding) =
                    dependency_function_return_binding_for_local_name(package, module, name)
            {
                return Some(binding);
            }

            dependency_value_root_binding_in_expr(package, module, callee, offset).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        dependency_value_root_binding_in_expr(package, module, expr, offset)
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        dependency_value_root_binding_in_expr(package, module, value, offset)
                    }
                })
            })
        }
        ql_ast::ExprKind::Question(inner) => match &inner.kind {
            ql_ast::ExprKind::Name(name)
                if dependency_struct_field_completion_span_contains(inner.span, offset) =>
            {
                dependency_global_question_binding_for_local_name(package, module, name)
            }
            ql_ast::ExprKind::Call { callee, .. }
                if matches!(&callee.kind, ql_ast::ExprKind::Name(_))
                    && dependency_struct_field_completion_span_contains(callee.span, offset) =>
            {
                let ql_ast::ExprKind::Name(name) = &callee.kind else {
                    unreachable!()
                };
                dependency_function_question_return_binding_for_local_name(package, module, name)
                    .or_else(|| {
                        dependency_global_question_binding_for_local_name(package, module, name)
                    })
            }
            _ => dependency_value_root_binding_in_expr(package, module, inner, offset),
        },
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => items
            .iter()
            .find_map(|expr| dependency_value_root_binding_in_expr(package, module, expr, offset)),
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_value_root_binding_in_block(package, module, block, offset)
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_value_root_binding_in_expr(package, module, condition, offset)
            .or_else(|| {
                dependency_value_root_binding_in_block(package, module, then_branch, offset)
            })
            .or_else(|| {
                else_branch.as_ref().and_then(|expr| {
                    dependency_value_root_binding_in_expr(package, module, expr, offset)
                })
            }),
        ql_ast::ExprKind::Match { value, arms } => {
            dependency_value_root_binding_in_expr(package, module, value, offset).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(|expr| {
                            dependency_value_root_binding_in_expr(package, module, expr, offset)
                        })
                        .or_else(|| {
                            dependency_value_root_binding_in_expr(
                                package, module, &arm.body, offset,
                            )
                        })
                })
            })
        }
        ql_ast::ExprKind::Closure { body, .. } => {
            dependency_value_root_binding_in_expr(package, module, body, offset)
        }
        ql_ast::ExprKind::Member { object, .. } | ql_ast::ExprKind::Unary { expr: object, .. } => {
            dependency_value_root_binding_in_expr(package, module, object, offset)
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            if dependency_indexed_iterable_target_contains(target, offset) {
                dependency_value_root_iterable_binding_in_expr(package, module, target, offset)
                    .or_else(|| {
                        dependency_struct_binding_for_bracket_expr(package, module, target, &[])
                    })
                    .or_else(|| {
                        dependency_value_root_binding_in_expr(package, module, target, offset)
                    })
            } else {
                dependency_value_root_binding_in_expr(package, module, target, offset).or_else(
                    || {
                        items.iter().find_map(|expr| {
                            dependency_value_root_binding_in_expr(package, module, expr, offset)
                        })
                    },
                )
            }
        }
        ql_ast::ExprKind::StructLiteral { path, fields } => {
            dependency_value_root_binding_for_path(package, module, path, offset).or_else(|| {
                fields.iter().find_map(|field| {
                    field.value.as_ref().and_then(|expr| {
                        dependency_value_root_binding_in_expr(package, module, expr, offset)
                    })
                })
            })
        }
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_value_root_binding_in_expr(package, module, left, offset)
                .or_else(|| dependency_value_root_binding_in_expr(package, module, right, offset))
        }
        ql_ast::ExprKind::Integer(_)
        | ql_ast::ExprKind::String { .. }
        | ql_ast::ExprKind::Bool(_)
        | ql_ast::ExprKind::NoneLiteral => None,
    }
}

fn render_dependency_type_expr(ty: &ql_ast::TypeExpr) -> String {
    match &ty.kind {
        ql_ast::TypeExprKind::Pointer { is_const, inner } => {
            let qualifier = if *is_const { "const " } else { "" };
            format!("*{}{}", qualifier, render_dependency_type_expr(inner))
        }
        ql_ast::TypeExprKind::Array { element, len } => {
            format!("[{}; {}]", render_dependency_type_expr(element), len)
        }
        ql_ast::TypeExprKind::Named { path, args } => {
            let mut rendered = path.segments.join(".");
            if !args.is_empty() {
                rendered.push('[');
                rendered.push_str(
                    &args
                        .iter()
                        .map(render_dependency_type_expr)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                rendered.push(']');
            }
            rendered
        }
        ql_ast::TypeExprKind::Tuple(items) => {
            let mut rendered = String::from("(");
            rendered.push_str(
                &items
                    .iter()
                    .map(render_dependency_type_expr)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            if items.len() == 1 {
                rendered.push(',');
            }
            rendered.push(')');
            rendered
        }
        ql_ast::TypeExprKind::Callable { params, ret } => format!(
            "({}) -> {}",
            params
                .iter()
                .map(render_dependency_type_expr)
                .collect::<Vec<_>>()
                .join(", "),
            render_dependency_type_expr(ret)
        ),
    }
}

pub fn parse_errors_to_diagnostics(errors: Vec<ParseError>) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| Diagnostic::error(error.message).with_label(Label::new(error.span)))
        .collect()
}

#[cfg(test)]
mod tests {
    use ql_hir::{ExprKind, ItemKind, StmtKind};

    use super::analyze_source;

    #[test]
    fn keeps_semantic_diagnostics_on_successful_analysis() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    return "oops"
}
"#,
        )
        .expect("type errors should still yield an analysis snapshot");

        assert!(analysis.has_errors());
        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.message == "return value has type mismatch: expected `Int`, found `String`"
        }));
    }

    #[test]
    fn exposes_expression_and_local_types_for_queries() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
        )
        .expect("source should analyze");

        let function = analysis
            .hir()
            .items
            .iter()
            .find_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ItemKind::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function should exist");
        let body = function.body.expect("main should have a body");
        let block = analysis.hir().block(body);
        let stmt_id = block.statements[0];
        let local_id = match &analysis.hir().stmt(stmt_id).kind {
            StmtKind::Let { pattern, .. } => match &analysis.hir().pattern(*pattern).kind {
                ql_hir::PatternKind::Binding(local_id) => *local_id,
                _ => panic!("expected binding pattern"),
            },
            _ => panic!("expected let statement"),
        };
        let return_expr = match &analysis.hir().stmt(block.statements[1]).kind {
            StmtKind::Return(Some(expr_id)) => *expr_id,
            _ => panic!("expected return statement with expression"),
        };

        assert_eq!(
            analysis
                .local_ty(local_id)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
        assert_eq!(
            analysis
                .expr_ty(return_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );

        let value_expr = match &analysis.hir().stmt(stmt_id).kind {
            StmtKind::Let { value, .. } => *value,
            _ => unreachable!(),
        };
        assert!(matches!(
            analysis.hir().expr(value_expr).kind,
            ExprKind::Integer(_)
        ));
        assert_eq!(
            analysis
                .expr_ty(value_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
    }

    #[test]
    fn exposes_array_and_index_types_for_queries() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let values = [1, 2, 3]
    let first = values[0]
    return first
}
"#,
        )
        .expect("source should analyze");

        let function = analysis
            .hir()
            .items
            .iter()
            .find_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ItemKind::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function should exist");
        let body = function.body.expect("main should have a body");
        let block = analysis.hir().block(body);

        let values_local = match &analysis.hir().stmt(block.statements[0]).kind {
            StmtKind::Let { pattern, .. } => match &analysis.hir().pattern(*pattern).kind {
                ql_hir::PatternKind::Binding(local_id) => *local_id,
                _ => panic!("expected binding pattern"),
            },
            _ => panic!("expected let statement"),
        };
        let (first_local, first_value_expr) = match &analysis.hir().stmt(block.statements[1]).kind {
            StmtKind::Let { pattern, value, .. } => {
                let local_id = match &analysis.hir().pattern(*pattern).kind {
                    ql_hir::PatternKind::Binding(local_id) => *local_id,
                    _ => panic!("expected binding pattern"),
                };
                (local_id, *value)
            }
            _ => panic!("expected second let statement"),
        };
        let return_expr = match &analysis.hir().stmt(block.statements[2]).kind {
            StmtKind::Return(Some(expr_id)) => *expr_id,
            _ => panic!("expected return statement with expression"),
        };

        assert_eq!(
            analysis
                .local_ty(values_local)
                .map(ToString::to_string)
                .as_deref(),
            Some("[Int; 3]")
        );
        assert_eq!(
            analysis
                .expr_ty(first_value_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
        assert_eq!(
            analysis
                .local_ty(first_local)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
        assert_eq!(
            analysis
                .expr_ty(return_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
    }

    #[test]
    fn exposes_tuple_hex_index_types_for_queries() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let pair = (1, "ql")
    let second = pair[0x1]
    return 0
}
"#,
        )
        .expect("source should analyze");

        let function = analysis
            .hir()
            .items
            .iter()
            .find_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ItemKind::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function should exist");
        let body = function.body.expect("main should have a body");
        let block = analysis.hir().block(body);

        let (second_local, second_value_expr) = match &analysis.hir().stmt(block.statements[1]).kind
        {
            StmtKind::Let { pattern, value, .. } => {
                let local_id = match &analysis.hir().pattern(*pattern).kind {
                    ql_hir::PatternKind::Binding(local_id) => *local_id,
                    _ => panic!("expected binding pattern"),
                };
                (local_id, *value)
            }
            _ => panic!("expected second let statement"),
        };

        assert_eq!(
            analysis
                .expr_ty(second_value_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("String")
        );
        assert_eq!(
            analysis
                .local_ty(second_local)
                .map(ToString::to_string)
                .as_deref(),
            Some("String")
        );
    }

    #[test]
    fn keeps_resolution_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    self
}
"#,
        )
        .expect("resolution errors should still yield an analysis snapshot");

        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.message == "invalid use of `self` outside a method receiver scope"
        }));
    }

    #[test]
    fn exposes_rendered_mir_for_debugging() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
        )
        .expect("source should analyze");

        let rendered = analysis.render_mir();
        assert!(rendered.contains("body 0 main"));
        assert!(rendered.contains("bind_pattern value <-"));
        assert!(rendered.contains("return"));
    }

    #[test]
    fn rendered_mir_exposes_explicit_closure_capture_facts() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    let make = move (extra) => value + extra
    return 0
}
"#,
        )
        .expect("source should analyze");

        let rendered = analysis.render_mir();
        assert!(rendered.contains("[captures: value@"));
    }

    #[test]
    fn exposes_rendered_ownership_for_debugging() {
        let analysis = analyze_source(
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json();
    return user.name
}
"#,
        )
        .expect("borrowck diagnostics should still yield an analysis snapshot");

        let rendered = analysis.render_borrowck();
        assert!(rendered.contains("ownership main"));
        assert!(rendered.contains("consume(move self into_json)"));
    }

    #[test]
    fn rendered_ownership_exposes_closure_escape_facts() {
        let analysis = analyze_source(
            r#"
fn apply(f: () -> Int) -> Int {
    return f()
}

fn main() -> Int {
    let value = 1
    let closure = move () => value
    return apply(closure)
}
"#,
        )
        .expect("source should analyze");

        let rendered = analysis.render_borrowck();
        assert!(rendered.contains("closures:"));
        assert!(rendered.contains("call-arg@"));
    }

    #[test]
    fn includes_borrowck_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#,
        )
        .expect("borrowck diagnostics should still yield an analysis snapshot");

        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message == "local `user` was used after move" })
        );
    }

    #[test]
    fn includes_deferred_cleanup_borrowck_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    defer user.name
    defer user.into_json()
    return ""
}
"#,
        )
        .expect("deferred cleanup diagnostics should still yield an analysis snapshot");

        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message == "local `user` was used after move" })
        );
    }

    #[test]
    fn includes_move_closure_capture_borrowck_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    let capture = move () => value
    return value
}
"#,
        )
        .expect("move closure diagnostics should still yield an analysis snapshot");

        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message == "local `value` was used after move" })
        );
        assert!(
            analysis
                .render_borrowck()
                .contains("consume(move closure capture)")
        );
    }
}
