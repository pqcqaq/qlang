use ql_ast::{ItemKind, Visibility};
use ql_parser::parse_interface_source;

#[test]
fn parses_interface_source_with_signature_only_items() {
    let source = r#"
package demo.dep

pub const DEFAULT_PORT: Int
pub static BUILD_ID: Int

pub fn exported() -> Int

pub struct Buffer[T] {
    value: T,
}

impl Buffer[Int] {
    pub fn len(self) -> Int
}

extend Buffer[Int] {
    pub fn twice(self) -> Int
}
"#;

    let module = parse_interface_source(source).expect("interface source should parse");
    assert_eq!(
        module.package.expect("package should exist").path.segments,
        vec!["demo", "dep"]
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Const(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Static(_)))
    );
    assert!(module.items.iter().any(|item| matches!(
        &item.kind,
        ItemKind::Function(function)
            if function.name == "exported"
                && function.body.is_none()
                && matches!(function.visibility, Visibility::Public)
    )));
    assert!(module.items.iter().any(|item| matches!(
        &item.kind,
        ItemKind::Impl(impl_block)
            if impl_block.methods.iter().any(|method| method.name == "len" && method.body.is_none())
    )));
    assert!(module.items.iter().any(|item| matches!(
        &item.kind,
        ItemKind::Extend(extend_block)
            if extend_block.methods.iter().any(|method| method.name == "twice" && method.body.is_none())
    )));
}
