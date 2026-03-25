#![allow(dead_code)]

use std::path::Path;

use ql_diagnostics::{Diagnostic, render_diagnostics};
use ql_parser::parse_source;

pub fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let ast = parse_source(source).expect("source should parse");
    let hir = ql_hir::lower_module(&ast);
    let resolution = ql_resolve::resolve_module(&hir);
    ql_typeck::check_module(&hir, &resolution)
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
