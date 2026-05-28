use ql_ast::{FunctionDecl, ItemKind, Module};

pub(super) fn parse_module(source: &str) -> Module {
    ql_parser::parse_source(source).expect("test source should parse")
}

pub(super) fn function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
    module
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == name => Some(function),
            _ => None,
        })
        .expect("test function should exist")
}
