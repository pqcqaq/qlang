use ql_parser::parse_source;

use crate::dependency_bridge_public_types::{
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
    dependency_public_type_bridge_order,
};

#[test]
fn dependency_public_struct_method_bridge_candidates_include_trait_impl_methods() {
    let module = parse_source(
        r#"
pub trait Reader {
    fn read(self) -> Int
}

pub struct Box {
    value: Int,
}

impl Reader for Box {
    pub fn read(self) -> Int {
        return self.value
    }
}
"#,
    )
    .expect("source should parse");

    let methods = dependency_public_struct_method_bridge_candidates(&module, "Box");
    let read = methods
        .get("read")
        .expect("trait receiver method should be bridgeable");

    assert_eq!(methods.len(), 1);
    assert_eq!(read.name, "read");
    assert!(read.body.is_some());
}

#[test]
fn dependency_public_type_bridge_order_supports_public_enum_payload_dependencies() {
    let module = parse_source(
        r#"
pub struct Issue {
    code: Int,
}

pub enum Status {
    Ready,
    Failed(Issue),
}
"#,
    )
    .expect("source should parse");

    let candidates = dependency_public_type_bridge_candidates(&module);
    let ordered = dependency_public_type_bridge_order("Status", &candidates)
        .expect("enum payload dependency order should resolve");

    assert_eq!(ordered, vec!["Issue".to_owned(), "Status".to_owned()]);
}
