use super::*;

#[test]
fn export_wrappers_include_public_free_functions() {
    let source = "pub fn add(value: Int) -> Int { return value + 1 }\n";

    let wrappers = render_public_dependency_function_export_wrappers_for_package("dep", source);

    assert!(wrappers.contains("extern \"c\" pub fn __ql_bridge_dep_add"));
    assert!(wrappers.contains("return add(value)"));
}

#[test]
fn export_wrappers_include_public_receiver_methods() {
    let source = r#"
pub struct Box { value: Int }

impl Box {
    pub fn read(self) -> Int {
        return self.value
    }
}
"#;

    let wrappers = render_public_dependency_function_export_wrappers_for_package("dep", source);

    assert!(wrappers.contains("extern \"c\" pub fn __ql_bridge_method_dep_Box_read"));
    assert!(wrappers.contains("return receiver.read()"));
}
