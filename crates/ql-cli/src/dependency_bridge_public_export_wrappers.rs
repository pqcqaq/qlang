use std::path::Path;

use ql_ast::ItemKind;
use ql_parser::parse_source;
use ql_project::{load_project_manifest, package_name};

use crate::build_plan::{PrepareProjectTargetBuildError, target_prep_dependency_manifest_failure};
use crate::dependency_bridge_modules::dependency_interface_module_import_path;
use crate::dependency_bridge_names::{
    render_dependency_public_function_export_wrapper,
    render_dependency_public_method_export_wrapper,
};
use crate::dependency_bridge_public_types::{
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
};

pub(crate) fn render_public_dependency_function_export_wrappers(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;
    let package_name = package_name(&manifest).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;
    Ok(render_public_dependency_function_export_wrappers_for_package(package_name, source))
}

pub(crate) fn render_public_dependency_function_export_wrappers_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let package_name = package_name(&manifest)
        .map_err(|error| target_prep_dependency_manifest_failure(Some(manifest_path), &error))?;
    Ok(render_public_dependency_function_export_wrappers_for_package(package_name, source))
}

fn render_public_dependency_function_export_wrappers_for_package(
    package_name: &str,
    source: &str,
) -> String {
    let Ok(module) = parse_source(source) else {
        return String::new();
    };

    let mut wrappers = Vec::new();
    let module_import_path = dependency_interface_module_import_path(package_name, &module);
    let type_candidates = dependency_public_type_bridge_candidates(&module);
    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        if let Some(wrapper) =
            render_dependency_public_function_export_wrapper(&module_import_path, function, source)
        {
            wrappers.push(wrapper);
        }
    }
    for struct_name in type_candidates.keys() {
        if type_candidates
            .get(struct_name)
            .is_some_and(|candidate| candidate.decl.is_generic())
        {
            continue;
        }
        for method in
            dependency_public_struct_method_bridge_candidates(&module, struct_name).values()
        {
            if let Some(wrapper) = render_dependency_public_method_export_wrapper(
                &module_import_path,
                struct_name,
                method,
                source,
            ) {
                wrappers.push(wrapper);
            }
        }
    }

    wrappers.join("\n\n")
}

#[cfg(test)]
mod tests {
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
}
