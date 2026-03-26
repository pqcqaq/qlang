#![allow(dead_code)]

use std::path::Path;

use ql_ast::Path as AstPath;
use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_hir::{Function, ItemId, ItemKind, Module};
use ql_parser::parse_source;
use ql_resolve::ResolutionMap;

pub fn resolved(source: &str) -> (Module, ResolutionMap) {
    let ast = parse_source(source).expect("source should parse");
    let hir = ql_hir::lower_module(&ast);
    let resolution = ql_resolve::resolve_module(&hir);
    (hir, resolution)
}

pub fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let (_, resolution) = resolved(source);
    resolution.diagnostics
}

pub fn diagnostic_messages(source: &str) -> Vec<String> {
    diagnostics(source)
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

pub fn rendered_diagnostics(source: &str) -> String {
    let diagnostics = diagnostics(source);
    render_diagnostics(Path::new("sample.ql"), source, &diagnostics)
}

pub fn path(segments: &[&str]) -> AstPath {
    AstPath::new(
        segments
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
    )
}

pub fn span_of(source: &str, needle: &str) -> ql_span::Span {
    span_of_nth(source, needle, 1)
}

pub fn span_of_nth(source: &str, needle: &str, occurrence: usize) -> ql_span::Span {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, matched)| ql_span::Span::new(start, start + matched.len()))
        .expect("needle occurrence should exist")
}

pub fn find_item_id(module: &Module, name: &str) -> ItemId {
    module
        .items
        .iter()
        .copied()
        .find(|&item_id| item_name(module, item_id).as_deref() == Some(name))
        .expect("named item should exist")
}

pub fn find_function<'module>(module: &'module Module, name: &str) -> &'module Function {
    let item_id = find_item_id(module, name);
    let item = module.item(item_id);
    let ItemKind::Function(function) = &item.kind else {
        panic!("named item should be a function");
    };
    function
}

pub fn find_impl_method<'module>(module: &'module Module, name: &str) -> &'module Function {
    module
        .items
        .iter()
        .find_map(|&item_id| match &module.item(item_id).kind {
            ItemKind::Impl(impl_block) => {
                impl_block.methods.iter().find(|method| method.name == name)
            }
            _ => None,
        })
        .expect("impl method should exist")
}

fn item_name(module: &Module, item_id: ItemId) -> Option<String> {
    match &module.item(item_id).kind {
        ItemKind::Function(function) => Some(function.name.clone()),
        ItemKind::Const(global) | ItemKind::Static(global) => Some(global.name.clone()),
        ItemKind::Struct(struct_decl) => Some(struct_decl.name.clone()),
        ItemKind::Enum(enum_decl) => Some(enum_decl.name.clone()),
        ItemKind::Trait(trait_decl) => Some(trait_decl.name.clone()),
        ItemKind::TypeAlias(alias) => Some(alias.name.clone()),
        ItemKind::Impl(_) | ItemKind::Extend(_) | ItemKind::ExternBlock(_) => None,
    }
}
