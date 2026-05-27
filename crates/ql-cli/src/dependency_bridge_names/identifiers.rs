pub(super) fn dependency_public_function_export_name(
    module_import_path: &[String],
    symbol_name: &str,
) -> String {
    let mut rendered = String::from("__ql_bridge_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered
}

pub(super) fn dependency_public_function_local_forwarder_name(
    module_import_path: &[String],
    symbol_name: &str,
) -> String {
    let mut rendered = String::from("__ql_bridge_local_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered
}

pub(super) fn dependency_public_method_export_name(
    module_import_path: &[String],
    struct_name: &str,
    symbol_name: &str,
) -> String {
    let mut rendered = String::from("__ql_bridge_method_");
    for segment in module_import_path {
        rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(segment));
        rendered.push('_');
    }
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(struct_name));
    rendered.push('_');
    rendered.push_str(&sanitize_dependency_bridge_identifier_fragment(symbol_name));
    rendered
}

fn sanitize_dependency_bridge_identifier_fragment(fragment: &str) -> String {
    let mut rendered = String::new();
    for ch in fragment.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            rendered.push(ch);
        } else {
            rendered.push('_');
        }
    }
    if rendered.is_empty() {
        rendered.push('_');
    }
    rendered
}
