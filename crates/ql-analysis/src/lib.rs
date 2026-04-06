mod query;
mod runtime;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ql_ast::{Item as AstItem, ItemKind as AstItemKind, Visibility as AstVisibility};
use ql_borrowck::{
    BorrowckResult, analyze_module as analyze_borrowck, render_result as render_borrowck_result,
};
use ql_diagnostics::{Diagnostic, Label, render_diagnostics};
use ql_hir::{ExprId, LocalId, PatternId, lower_module};
use ql_mir::{MirModule, lower_module as lower_mir, render_module as render_mir_module};
use ql_parser::{ParseError, parse_source};
use ql_project::{
    InterfaceArtifact, InterfaceError, InterfaceModule, ProjectError, ProjectManifest,
    collect_package_sources, default_interface_path, load_interface_artifact,
    load_project_manifest, load_reference_manifests,
};
use ql_resolve::{ImportBinding, ResolutionMap, resolve_module};
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
    pub source_path: String,
    pub kind: SymbolKind,
    pub name: String,
    pub detail: String,
    pub path: PathBuf,
    pub definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyImportOccurrence {
    local_name: String,
    span: Span,
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
        let module = parse_source(source).ok()?;
        let root_offset = dependency_variant_completion_root_offset(source, offset)?;
        let root_end = dependency_identifier_end(source, root_offset);
        let root_name = source.get(root_offset..root_end)?;
        let (dependency, symbol) =
            dependency_import_binding_for_local_name(self, &module, root_name)?;
        dependency.variant_completions_for(symbol)
    }

    pub fn dependency_struct_field_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let module = parse_source(source).ok()?;
        let site = dependency_struct_field_completion_site(&module, offset)?;
        let (dependency, symbol) =
            dependency_struct_import_binding_for_local_name(self, &module, &site.root_name)?;
        let mut items = dependency
            .struct_decl_for(symbol)?
            .fields
            .iter()
            .filter(|field| {
                !site
                    .excluded_field_names
                    .iter()
                    .any(|name| name == &field.name)
            })
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

    pub fn dependency_method_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let module = parse_source(source).ok()?;
        let binding = dependency_member_completion_binding(
            self,
            &module,
            source,
            offset,
            DependencyMemberCompletionKind::Method,
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

    pub fn dependency_member_field_completions_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let module = parse_source(source).ok()?;
        let binding = dependency_member_completion_binding(
            self,
            &module,
            source,
            offset,
            DependencyMemberCompletionKind::Field,
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
        let module = parse_source(source).ok()?;
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
        let module = parse_source(source).ok()?;
        let target = self.dependency_method_target_in_source_at(source, offset)?;
        let mut references = self
            .dependency_method_occurrences(&module)
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

    pub fn dependency_struct_field_references_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<Vec<ReferenceTarget>> {
        let module = parse_source(source).ok()?;
        let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
        let mut references = self
            .dependency_struct_field_occurrences(&module)
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
        let module = parse_source(source).ok()?;
        let binding = dependency_member_completion_binding(
            self,
            &module,
            source,
            offset,
            DependencyMemberCompletionKind::ValueType,
        )?;
        Some(DependencyDefinitionTarget {
            package_name: binding.package_name,
            source_path: binding.source_path,
            kind: SymbolKind::Struct,
            name: binding.struct_name,
            path: binding.path,
            span: binding.definition_span,
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
        let module = parse_source(source).ok()?;
        let (root_offset, _, variant_name) = dependency_variant_reference_at(source, offset)?;
        let root_end = dependency_identifier_end(source, root_offset);
        let root_name = source.get(root_offset..root_end)?;
        let (dependency, symbol) =
            dependency_import_binding_for_local_name(self, &module, root_name)?;
        dependency.variant_for(symbol, &variant_name)?;
        let definition_span = dependency.artifact_span_for(symbol)?;
        Some(DependencyDefinitionTarget {
            package_name: dependency.artifact.package_name.clone(),
            source_path: symbol.source_path.clone(),
            kind: SymbolKind::Enum,
            name: symbol.name.clone(),
            path: dependency.interface_path.clone(),
            span: definition_span,
        })
    }

    pub fn dependency_struct_field_type_definition_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let target = self.dependency_struct_field_target_in_source_at(source, offset)?;
        let module = parse_source(source).ok()?;
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
        if dependency_question_wrapped_field_reference_in_module(&module, offset) {
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
        let module = parse_source(source).ok()?;
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
        if dependency_question_wrapped_method_reference_in_module(&module, offset) {
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
        let module = parse_source(source).ok()?;
        let target_occurrence = dependency_import_occurrence_in_module(&module, offset)?;
        let (dependency, symbol) =
            dependency_import_binding_for_local_name(self, &module, &target_occurrence.local_name)?;
        let mut references = source
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
            .collect::<Vec<_>>();
        if references.is_empty() {
            return None;
        }
        references.sort_by_key(|reference| (reference.span.start, reference.span.end));
        Some(references)
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
        let module = parse_source(source).ok()?;
        let occurrence = dependency_import_occurrence_in_module(&module, offset)?;
        let (dependency, symbol) =
            dependency_import_binding_for_local_name(self, &module, &occurrence.local_name)?;
        let definition_span = dependency.artifact_span_for(symbol)?;
        Some(DependencyResolvedTarget {
            import_span: occurrence.span,
            package_name: dependency.artifact.package_name.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            detail: symbol.detail.clone(),
            path: dependency.interface_path.clone(),
            definition_span,
        })
    }

    fn dependency_type_target_in_source_at(
        &self,
        source: &str,
        offset: usize,
    ) -> Option<DependencyResolvedTarget> {
        let module = parse_source(source).ok()?;
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
        Some(DependencyResolvedTarget {
            import_span: occurrence.span,
            package_name: dependency.artifact.package_name.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            detail: symbol.detail.clone(),
            path: dependency.interface_path.clone(),
            definition_span,
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
        let module = parse_source(source).ok()?;
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
            source_path: symbol.source_path.clone(),
            enum_name: symbol.name.clone(),
            name: variant.name.clone(),
            detail: dependency_variant_detail(&symbol.name, variant),
            path: dependency.interface_path.clone(),
            definition_span,
        })
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
        let module = parse_source(source).ok()?;
        self.dependency_struct_field_occurrences(&module)
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
            .map(|occurrence| DependencyStructFieldTarget {
                reference_span: occurrence.reference_span,
                package_name: occurrence.package_name,
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
        let module = parse_source(source).ok()?;
        self.dependency_method_occurrences(&module)
            .into_iter()
            .find(|occurrence| occurrence.reference_span.contains(offset))
            .map(|occurrence| DependencyMethodTarget {
                reference_span: occurrence.reference_span,
                package_name: occurrence.package_name,
                source_path: occurrence.source_path,
                struct_name: occurrence.struct_name,
                name: occurrence.name,
                detail: occurrence.detail,
                path: occurrence.path,
                definition_span: occurrence.definition_span,
            })
    }

    fn dependency_struct_field_occurrences(
        &self,
        module: &ql_ast::Module,
    ) -> Vec<DependencyStructFieldOccurrence> {
        let mut occurrences = Vec::new();
        let mut scopes = vec![HashMap::new()];
        for item in &module.items {
            collect_dependency_struct_field_occurrences_in_item(
                self,
                module,
                item,
                &mut scopes,
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
        for item in &module.items {
            collect_dependency_method_occurrences_in_item(
                self,
                module,
                item,
                &mut scopes,
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
    source_path: String,
    struct_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyMethodOccurrence {
    reference_span: Span,
    package_name: String,
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
    source_path: String,
    struct_name: String,
    name: String,
    detail: String,
    path: PathBuf,
    definition_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DependencyStructBinding {
    package_name: String,
    source_path: String,
    struct_name: String,
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

    fn import_binding_at(&self, offset: usize) -> Option<(ImportBinding, Span)> {
        self.index.import_binding_at(offset)
    }

    fn type_import_binding_at(&self, offset: usize) -> Option<ImportBinding> {
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
    /// - ambiguous member surfaces, parse-error tolerant completion, and cross-file project indexing
    ///   are still intentionally deferred
    pub fn completions_at(&self, offset: usize) -> Option<Vec<CompletionItem>> {
        self.index.completions_at(offset)
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

    pub fn render_diagnostics(&self, path: &Path, source: &str) -> String {
        render_diagnostics(path, source, self.diagnostics())
    }

    pub fn render_mir(&self) -> String {
        render_mir_module(&self.mir, &self.hir)
    }

    pub fn render_borrowck(&self) -> String {
        render_borrowck_result(&self.borrowck, &self.mir)
    }
}

/// Analyze one source string. Parse failures are returned as diagnostics directly.
/// Resolution and type diagnostics are stored on the returned [`Analysis`] even when errors exist.
pub fn analyze_source(source: &str) -> Result<Analysis, Vec<Diagnostic>> {
    let ast = parse_source(source).map_err(parse_errors_to_diagnostics)?;
    let hir = lower_module(&ast);
    let resolution = resolve_module(&hir);
    let mir = lower_mir(&hir, &resolution);
    let typeck = analyze_types(&hir, &resolution);
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
        ql_ast::PatternKind::Tuple(items) | ql_ast::PatternKind::TupleStruct { items, .. } => items
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
    let [root_name] = path.segments.as_slice() else {
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
    let [root_name] = path.segments.as_slice() else {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum DependencyMemberCompletionKind {
    Field,
    Method,
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
    for item in &module.items {
        if let Some(binding) = dependency_member_completion_binding_in_item(
            package,
            module,
            item,
            source,
            offset,
            kind,
            &mut scopes,
        ) {
            return Some(binding);
        }
    }
    None
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
) -> Option<DependencyStructBinding> {
    let body = function.body.as_ref()?;
    scopes.push(HashMap::new());
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
        if binding.is_some() {
            scopes.pop();
            return binding;
        }
    }
    let binding = dependency_member_completion_binding_in_block(
        package, module, body, source, offset, kind, scopes,
    );
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
) -> Option<DependencyStructBinding> {
    match &item.kind {
        AstItemKind::Function(function) => dependency_member_completion_binding_in_function(
            package, module, function, None, source, offset, kind, scopes,
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
            )
        }
        AstItemKind::Struct(struct_decl) => struct_decl.fields.iter().find_map(|field| {
            field.default.as_ref().and_then(|default| {
                dependency_member_completion_binding_in_expr(
                    package, module, default, source, offset, kind, scopes,
                )
            })
        }),
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                let binding = dependency_member_completion_binding_in_function(
                    package, module, method, None, source, offset, kind, scopes,
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
) -> Option<DependencyStructBinding> {
    scopes.push(HashMap::new());
    for stmt in &block.statements {
        if let Some(binding) = dependency_member_completion_binding_in_stmt(
            package, module, stmt, source, offset, kind, scopes,
        ) {
            scopes.pop();
            return Some(binding);
        }
    }
    let binding = block.tail.as_ref().and_then(|tail| {
        dependency_member_completion_binding_in_expr(
            package, module, tail, source, offset, kind, scopes,
        )
    });
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
) -> Option<DependencyStructBinding> {
    match &stmt.kind {
        ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } => {
            let expr_binding = dependency_member_completion_binding_in_expr(
                package, module, value, source, offset, kind, scopes,
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
            bind_dependency_struct_let(package, module, pattern, ty.as_ref(), value, scopes);
            expr_binding.or(pattern_binding)
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => dependency_member_completion_binding_in_expr(
            package, module, expr, source, offset, kind, scopes,
        ),
        ql_ast::StmtKind::While { condition, body } => {
            dependency_member_completion_binding_in_expr(
                package, module, condition, source, offset, kind, scopes,
            )
            .or_else(|| {
                dependency_member_completion_binding_in_block(
                    package, module, body, source, offset, kind, scopes,
                )
            })
        }
        ql_ast::StmtKind::Loop { body } => dependency_member_completion_binding_in_block(
            package, module, body, source, offset, kind, scopes,
        ),
        ql_ast::StmtKind::For { iterable, body, .. } => {
            dependency_member_completion_binding_in_expr(
                package, module, iterable, source, offset, kind, scopes,
            )
            .or_else(|| {
                dependency_member_completion_binding_in_block(
                    package, module, body, source, offset, kind, scopes,
                )
            })
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
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Tuple(items) | ql_ast::ExprKind::Array(items) => {
            items.iter().find_map(|item| {
                dependency_member_completion_binding_in_expr(
                    package, module, item, source, offset, kind, scopes,
                )
            })
        }
        ql_ast::ExprKind::Block(block) | ql_ast::ExprKind::Unsafe(block) => {
            dependency_member_completion_binding_in_block(
                package, module, block, source, offset, kind, scopes,
            )
        }
        ql_ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => dependency_member_completion_binding_in_expr(
            package, module, condition, source, offset, kind, scopes,
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
            )
        })
        .or_else(|| {
            else_branch.as_ref().and_then(|expr| {
                dependency_member_completion_binding_in_expr(
                    package, module, expr, source, offset, kind, scopes,
                )
            })
        }),
        ql_ast::ExprKind::Match { value, arms } => dependency_member_completion_binding_in_expr(
            package, module, value, source, offset, kind, scopes,
        )
        .or_else(|| {
            let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
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
                let binding = pattern_binding.or_else(|| {
                    arm.guard
                        .as_ref()
                        .and_then(|guard| {
                            dependency_member_completion_binding_in_expr(
                                package, module, guard, source, offset, kind, scopes,
                            )
                        })
                        .or_else(|| {
                            dependency_member_completion_binding_in_expr(
                                package, module, &arm.body, source, offset, kind, scopes,
                            )
                        })
                });
                scopes.pop();
                if binding.is_some() {
                    return binding;
                }
            }
            None
        }),
        ql_ast::ExprKind::Closure { params, body, .. } => {
            scopes.push(HashMap::new());
            for param in params {
                let binding = dependency_member_completion_binding_for_closure_param(
                    package, module, param, offset, kind,
                );
                bind_dependency_struct_closure_param(package, module, param, scopes);
                if binding.is_some() {
                    scopes.pop();
                    return binding;
                }
            }
            let binding = dependency_member_completion_binding_in_expr(
                package, module, body, source, offset, kind, scopes,
            );
            scopes.pop();
            binding
        }
        ql_ast::ExprKind::Call { callee, args } => dependency_member_completion_binding_in_expr(
            package, module, callee, source, offset, kind, scopes,
        )
        .or_else(|| {
            args.iter().find_map(|arg| match arg {
                ql_ast::CallArg::Positional(expr) => dependency_member_completion_binding_in_expr(
                    package, module, expr, source, offset, kind, scopes,
                ),
                ql_ast::CallArg::Named { value, .. } => {
                    dependency_member_completion_binding_in_expr(
                        package, module, value, source, offset, kind, scopes,
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
            package, module, object, source, offset, kind, scopes,
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
            dependency_member_completion_binding_in_expr(
                package, module, target, source, offset, kind, scopes,
            )
            .or_else(|| {
                items.iter().find_map(|item| {
                    dependency_member_completion_binding_in_expr(
                        package, module, item, source, offset, kind, scopes,
                    )
                })
            })
        }
        ql_ast::ExprKind::StructLiteral { fields, .. } => fields.iter().find_map(|field| {
            field.value.as_ref().and_then(|value| {
                dependency_member_completion_binding_in_expr(
                    package, module, value, source, offset, kind, scopes,
                )
            })
        }),
        ql_ast::ExprKind::Binary { left, right, .. } => {
            dependency_member_completion_binding_in_expr(
                package, module, left, source, offset, kind, scopes,
            )
            .or_else(|| {
                dependency_member_completion_binding_in_expr(
                    package, module, right, source, offset, kind, scopes,
                )
            })
        }
        ql_ast::ExprKind::Unary { expr, .. } | ql_ast::ExprKind::Question(expr) => {
            dependency_member_completion_binding_in_expr(
                package, module, expr, source, offset, kind, scopes,
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
        ql_ast::PatternKind::Tuple(items) => items
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

fn collect_dependency_method_occurrences_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    match &item.kind {
        AstItemKind::Function(function) => {
            if let Some(body) = &function.body {
                scopes.push(HashMap::new());
                for param in &function.params {
                    bind_dependency_struct_param(package, module, param, None, scopes);
                }
                collect_dependency_method_occurrences_in_block(
                    package,
                    module,
                    body,
                    scopes,
                    occurrences,
                );
                scopes.pop();
            }
        }
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                &global.value,
                scopes,
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
                        occurrences,
                    );
                }
            }
        }
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(package, module, param, None, scopes);
                    }
                    collect_dependency_method_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        occurrences,
                    );
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
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                    }
                    collect_dependency_method_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        occurrences,
                    );
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
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                    }
                    collect_dependency_method_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        occurrences,
                    );
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
    occurrences: &mut Vec<DependencyMethodOccurrence>,
) {
    scopes.push(HashMap::new());
    for stmt in &block.statements {
        collect_dependency_method_occurrences_in_stmt(package, module, stmt, scopes, occurrences);
    }
    if let Some(tail) = &block.tail {
        collect_dependency_method_occurrences_in_expr(package, module, tail, scopes, occurrences);
    }
    scopes.pop();
}

fn collect_dependency_method_occurrences_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
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
                occurrences,
            );
            bind_dependency_struct_let(package, module, pattern, ty.as_ref(), value, scopes);
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::While { condition, body } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                condition,
                scopes,
                occurrences,
            );
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Loop { body } => {
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::For { iterable, body, .. } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                iterable,
                scopes,
                occurrences,
            );
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {}
    }
}

fn collect_dependency_method_occurrences_in_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
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
                occurrences,
            );
            collect_dependency_method_occurrences_in_block(
                package,
                module,
                then_branch,
                scopes,
                occurrences,
            );
            if let Some(expr) = else_branch {
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    expr,
                    scopes,
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
                occurrences,
            );
            let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
                if let Some(binding) = &value_binding {
                    bind_dependency_struct_match_pattern(package, &arm.pattern, binding, scopes);
                }
                if let Some(guard) = &arm.guard {
                    collect_dependency_method_occurrences_in_expr(
                        package,
                        module,
                        guard,
                        scopes,
                        occurrences,
                    );
                }
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    &arm.body,
                    scopes,
                    occurrences,
                );
                scopes.pop();
            }
        }
        ql_ast::ExprKind::Closure { params, body, .. } => {
            scopes.push(HashMap::new());
            for param in params {
                bind_dependency_struct_closure_param(package, module, param, scopes);
            }
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
            scopes.pop();
        }
        ql_ast::ExprKind::Call { callee, args } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                callee,
                scopes,
                occurrences,
            );
            for arg in args {
                match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        collect_dependency_method_occurrences_in_expr(
                            package,
                            module,
                            expr,
                            scopes,
                            occurrences,
                        );
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        collect_dependency_method_occurrences_in_expr(
                            package,
                            module,
                            value,
                            scopes,
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
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                object,
                scopes,
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
        ql_ast::ExprKind::Bracket { target, items } => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                target,
                scopes,
                occurrences,
            );
            for item in items {
                collect_dependency_method_occurrences_in_expr(
                    package,
                    module,
                    item,
                    scopes,
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
                occurrences,
            );
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                right,
                scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Unary { expr, .. } | ql_ast::ExprKind::Question(expr) => {
            collect_dependency_method_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
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

fn collect_dependency_struct_field_occurrences_in_item(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    item: &ql_ast::Item,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    match &item.kind {
        AstItemKind::Function(function) => {
            if let Some(body) = &function.body {
                scopes.push(HashMap::new());
                for param in &function.params {
                    bind_dependency_struct_param(package, module, param, None, scopes);
                }
                collect_dependency_struct_field_occurrences_in_block(
                    package,
                    module,
                    body,
                    scopes,
                    occurrences,
                );
                scopes.pop();
            }
        }
        AstItemKind::Const(global) | AstItemKind::Static(global) => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                &global.value,
                scopes,
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
                        occurrences,
                    );
                }
            }
        }
        AstItemKind::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                if let Some(body) = &method.body {
                    scopes.push(HashMap::new());
                    for param in &method.params {
                        bind_dependency_struct_param(package, module, param, None, scopes);
                    }
                    collect_dependency_struct_field_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        occurrences,
                    );
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
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                    }
                    collect_dependency_struct_field_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        occurrences,
                    );
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
                    for param in &method.params {
                        bind_dependency_struct_param(
                            package,
                            module,
                            param,
                            receiver_binding.as_ref(),
                            scopes,
                        );
                    }
                    collect_dependency_struct_field_occurrences_in_block(
                        package,
                        module,
                        body,
                        scopes,
                        occurrences,
                    );
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
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    scopes.push(HashMap::new());
    for stmt in &block.statements {
        collect_dependency_struct_field_occurrences_in_stmt(
            package,
            module,
            stmt,
            scopes,
            occurrences,
        );
    }
    if let Some(tail) = &block.tail {
        collect_dependency_struct_field_occurrences_in_expr(
            package,
            module,
            tail,
            scopes,
            occurrences,
        );
    }
    scopes.pop();
}

fn collect_dependency_struct_field_occurrences_in_stmt(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    stmt: &ql_ast::Stmt,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
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
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                value,
                scopes,
                occurrences,
            );
            bind_dependency_struct_let(package, module, pattern, ty.as_ref(), value, scopes);
        }
        ql_ast::StmtKind::Return(Some(expr))
        | ql_ast::StmtKind::Defer(expr)
        | ql_ast::StmtKind::Expr { expr, .. } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::While { condition, body } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                condition,
                scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Loop { body } => {
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            collect_dependency_struct_field_occurrences_in_pattern(
                package,
                module,
                pattern,
                scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                iterable,
                scopes,
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
        }
        ql_ast::StmtKind::Return(None) | ql_ast::StmtKind::Break | ql_ast::StmtKind::Continue => {}
    }
}

fn collect_dependency_struct_field_occurrences_in_pattern(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    pattern: &ql_ast::Pattern,
    scopes: &mut Vec<HashMap<String, DependencyStructBinding>>,
    occurrences: &mut Vec<DependencyStructFieldOccurrence>,
) {
    match &pattern.kind {
        ql_ast::PatternKind::Tuple(items) | ql_ast::PatternKind::TupleStruct { items, .. } => {
            for item in items {
                collect_dependency_struct_field_occurrences_in_pattern(
                    package,
                    module,
                    item,
                    scopes,
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
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_block(
                package,
                module,
                then_branch,
                scopes,
                occurrences,
            );
            if let Some(expr) = else_branch {
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    expr,
                    scopes,
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
                occurrences,
            );
            let value_binding = dependency_struct_binding_for_expr(package, module, value, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
                if let Some(binding) = &value_binding {
                    bind_dependency_struct_match_pattern(package, &arm.pattern, binding, scopes);
                }
                collect_dependency_struct_field_occurrences_in_pattern(
                    package,
                    module,
                    &arm.pattern,
                    scopes,
                    occurrences,
                );
                if let Some(guard) = &arm.guard {
                    collect_dependency_struct_field_occurrences_in_expr(
                        package,
                        module,
                        guard,
                        scopes,
                        occurrences,
                    );
                }
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    &arm.body,
                    scopes,
                    occurrences,
                );
                scopes.pop();
            }
        }
        ql_ast::ExprKind::Closure { params, body, .. } => {
            scopes.push(HashMap::new());
            for param in params {
                bind_dependency_struct_closure_param(package, module, param, scopes);
            }
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                body,
                scopes,
                occurrences,
            );
            scopes.pop();
        }
        ql_ast::ExprKind::Call { callee, args } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                callee,
                scopes,
                occurrences,
            );
            for arg in args {
                match arg {
                    ql_ast::CallArg::Positional(expr) => {
                        collect_dependency_struct_field_occurrences_in_expr(
                            package,
                            module,
                            expr,
                            scopes,
                            occurrences,
                        );
                    }
                    ql_ast::CallArg::Named { value, .. } => {
                        collect_dependency_struct_field_occurrences_in_expr(
                            package,
                            module,
                            value,
                            scopes,
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
                occurrences,
            );
            if let Some(binding) =
                dependency_struct_binding_for_expr(package, module, object, scopes)
            {
                if !binding.methods.contains_key(field) {
                    push_dependency_struct_field_occurrence_for_binding(
                        &binding,
                        field,
                        *field_span,
                        occurrences,
                    );
                }
            }
        }
        ql_ast::ExprKind::Question(object) => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                object,
                scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Bracket { target, items } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                target,
                scopes,
                occurrences,
            );
            for item in items {
                collect_dependency_struct_field_occurrences_in_expr(
                    package,
                    module,
                    item,
                    scopes,
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
                occurrences,
            );
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                right,
                scopes,
                occurrences,
            );
        }
        ql_ast::ExprKind::Unary { expr, .. } => {
            collect_dependency_struct_field_occurrences_in_expr(
                package,
                module,
                expr,
                scopes,
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
    let [root_name] = path.segments.as_slice() else {
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
                },
            ))
        })
        .collect();
    let methods = dependency.struct_methods_for(symbol);

    Some(DependencyStructBinding {
        package_name: dependency.artifact.package_name.clone(),
        source_path: symbol.source_path.clone(),
        struct_name: symbol.name.clone(),
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

fn dependency_struct_binding_for_name(
    scopes: &[HashMap<String, DependencyStructBinding>],
    name: &str,
) -> Option<DependencyStructBinding> {
    scopes
        .iter()
        .rev()
        .find_map(|scope| scope.get(name).cloned())
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
    let ql_ast::ExprKind::Member { object, field, .. } = &callee.kind else {
        return None;
    };
    let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
    let method = binding.methods.get(field)?;
    let return_type = method.return_type_definition.as_ref()?;
    dependency_struct_binding_for_definition_target(package, return_type)
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
        ql_ast::ExprKind::Member { object, field, .. } => {
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            let field = binding.fields.get(field)?;
            let type_definition = field.question_type_definition.as_ref()?;
            dependency_struct_binding_for_definition_target(package, type_definition)
        }
        ql_ast::ExprKind::Call { callee, .. } => {
            let ql_ast::ExprKind::Member { object, field, .. } = &callee.kind else {
                return None;
            };
            let binding = dependency_struct_binding_for_expr(package, module, object, scopes)?;
            let method = binding.methods.get(field)?;
            let return_type = method.question_return_type_definition.as_ref()?;
            dependency_struct_binding_for_definition_target(package, return_type)
        }
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
    for stmt in &block.statements {
        if let ql_ast::StmtKind::Let {
            pattern, ty, value, ..
        } = &stmt.kind
        {
            bind_dependency_struct_let(package, module, pattern, ty.as_ref(), value, &mut scopes);
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

fn dependency_struct_binding_for_expr(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    expr: &ql_ast::Expr,
    scopes: &[HashMap<String, DependencyStructBinding>],
) -> Option<DependencyStructBinding> {
    match &expr.kind {
        ql_ast::ExprKind::Name(name) => dependency_struct_binding_for_name(scopes, name),
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

fn bind_dependency_struct_let(
    package: &PackageAnalysis,
    module: &ql_ast::Module,
    pattern: &ql_ast::Pattern,
    ty: Option<&ql_ast::TypeExpr>,
    value: &ql_ast::Expr,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
) {
    let binding = ty
        .and_then(|ty| dependency_struct_binding_for_type_expr(package, module, ty))
        .or_else(|| dependency_struct_binding_for_expr(package, module, value, scopes));
    let Some(binding) = binding else {
        return;
    };
    bind_dependency_struct_pattern(package, pattern, &binding, scopes);
}

fn bind_dependency_struct_match_pattern(
    package: &PackageAnalysis,
    pattern: &ql_ast::Pattern,
    binding: &DependencyStructBinding,
    scopes: &mut [HashMap<String, DependencyStructBinding>],
) {
    bind_dependency_struct_pattern(package, pattern, binding, scopes);
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
