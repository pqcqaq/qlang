mod query;
mod runtime;

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
    AsyncContextInfo, AsyncOperatorKind, CompletionItem, DefinitionTarget, HoverInfo,
    LoopControlContextInfo, LoopControlKind, ReferenceTarget, RenameEdit, RenameError,
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

    fn artifact_span_for(&self, symbol: &DependencySymbol) -> Option<Span> {
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
            if module.source_path == symbol.source_path {
                return Some(Span::new(
                    module_index + symbol.span.start,
                    module_index + symbol.span.end,
                ));
            }
            search_start = module_index + module.contents.len();
        }
        None
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

    pub fn dependency_hover_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyHoverInfo> {
        let (dependency, symbol, import_span) =
            self.resolve_dependency_import_target(analysis, offset)?;
        Some(DependencyHoverInfo {
            span: import_span,
            package_name: dependency.artifact.package_name.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            detail: symbol.detail.clone(),
        })
    }

    pub fn dependency_definition_at(
        &self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<DependencyDefinitionTarget> {
        let (dependency, symbol, _) = self.resolve_dependency_import_target(analysis, offset)?;
        let span = dependency.artifact_span_for(symbol)?;
        Some(DependencyDefinitionTarget {
            package_name: dependency.artifact.package_name.clone(),
            source_path: symbol.source_path.clone(),
            kind: symbol.kind,
            name: symbol.name.clone(),
            path: dependency.interface_path.clone(),
            span,
        })
    }

    fn resolve_dependency_import_target<'a>(
        &'a self,
        analysis: &Analysis,
        offset: usize,
    ) -> Option<(&'a DependencyInterface, &'a DependencySymbol, Span)> {
        let (binding, import_span) = analysis.import_binding_at(offset)?;
        let imported_name = binding.path.segments.last()?;
        let mut matches = self
            .dependencies
            .iter()
            .filter(|dependency| dependency_matches_import(dependency, &binding))
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
        let (dependency, symbol) = matches.pop()?;
        Some((dependency, symbol, import_span))
    }
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

    let dependency_manifests =
        load_reference_manifests(&manifest).map_err(PackageAnalysisError::Project)?;
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

    Ok(PackageAnalysis {
        manifest,
        modules,
        dependencies,
    })
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

    let manifest_name = dependency
        .manifest
        .package
        .as_ref()
        .map(|package| package.name.as_str());
    prefix_segments
        .iter()
        .any(|segment| segment == &dependency.artifact.package_name)
        || manifest_name.is_some_and(|name| prefix_segments.iter().any(|segment| segment == name))
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
