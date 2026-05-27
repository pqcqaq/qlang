use std::collections::{BTreeMap, BTreeSet};

use ql_parser::parse_source;

use super::*;

#[test]
fn module_method_forwarders_follow_required_type_dependencies() {
    let dependency_source = r#"
pub struct Box { value: Int }
pub struct Label { value: Int }

impl Box {
    pub fn label(self) -> Label {
        return Label { value: self.value }
    }
}

impl Label {
    pub fn read(self) -> Int {
        return self.value
    }
}
"#;
    let dependency_module = parse_source(dependency_source).unwrap();
    let module_import_path = dependency_interface_module_import_path("dep", &dependency_module);
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    required_types_by_module_path
        .entry(module_import_path.clone())
        .or_default()
        .insert("Box".to_owned());
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut forwarders = Vec::new();

    collect_dependency_module_public_method_forwarders(
        "dep",
        &dependency_module,
        dependency_source,
        &required_types_by_module_path,
        &mut discovered_required_types,
        &mut forwarders,
    );

    assert_eq!(
        discovered_required_types.get(&module_import_path),
        Some(&BTreeSet::from(["Label".to_owned()]))
    );
    assert_eq!(forwarders.len(), 2);
    let rendered = forwarders.join("\n\n");
    assert!(rendered.contains("pub fn label(self) -> Label"));
    assert!(rendered.contains("pub fn read(self) -> Int"));
}
