use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ql_parser::parse_source;
use ql_project::{WorkspaceBuildTargets, load_project_manifest};

use crate::build_plan::report_project_build_dependency_error;
use crate::build_source_rewrites::{
    RenderedDependencyBridgeItems, join_dependency_bridge_sections,
};
use crate::cli_utils::normalize_path;
use crate::dependency_bridge_imports::{
    collect_imported_dependency_externs, collect_top_level_definition_names,
};
use crate::dependency_bridge_modules::{
    dependency_generic_specialization_module_refs, dependency_generic_specialization_modules,
    dependency_interface_module_import_path, package_under_test_bridge_modules,
};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_public_function_errors::report_package_under_test_function_forwarder_error;
use crate::dependency_bridge_public_functions::collect_dependency_module_public_function_forwarders;
use crate::dependency_bridge_public_type_declarations::{
    DependencyPublicTypeBridgeError, collect_dependency_module_public_type_declarations,
};
use crate::dependency_bridge_reporting::{
    report_package_under_test_local_conflict, report_package_under_test_symbol_conflict,
};

pub(crate) fn render_package_under_test_bridge_items(
    command_label: &str,
    workspace_members: &[WorkspaceBuildTargets],
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyBridgeItems, u8> {
    let Some(member) = workspace_members.iter().find(|member| {
        normalize_path(&member.member_manifest_path) == normalize_path(manifest_path)
    }) else {
        return Ok(RenderedDependencyBridgeItems::default());
    };
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyBridgeItems::default()),
    };
    let bridge_modules = package_under_test_bridge_modules(command_label, member, report_failure)?;
    if bridge_modules.is_empty() {
        return Ok(RenderedDependencyBridgeItems::default());
    }

    let package_name = member.package_name.as_str();
    let owner_manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let specialization_modules =
        dependency_generic_specialization_modules(command_label, &owner_manifest, report_failure)?;
    let specialization_modules =
        dependency_generic_specialization_module_refs(&specialization_modules);
    let module_import_paths = bridge_modules
        .iter()
        .map(|module| dependency_interface_module_import_path(package_name, &module.module))
        .collect::<BTreeSet<_>>();
    let imported_externs =
        collect_imported_dependency_externs(&root_source_module, &module_import_paths);
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut function_owners = BTreeMap::<String, DependencyExternOwner>::new();
    let mut rendered_specializations = BTreeSet::new();
    for module in &bridge_modules {
        collect_dependency_module_public_function_forwarders(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            &root_source_module,
            Some(&imported_externs),
            None,
            &occupied_root_names,
            &specialization_modules,
            &mut required_types_by_module_path,
            &mut function_owners,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
        )
        .map_err(|error| {
            if !report_failure {
                return 1;
            }
            report_package_under_test_function_forwarder_error(command_label, package_name, error);
            1
        })?;
    }

    let mut type_declarations = Vec::new();
    let mut type_owners = BTreeMap::<String, DependencyExternOwner>::new();
    for module in &bridge_modules {
        let module_import_path =
            dependency_interface_module_import_path(package_name, &module.module);
        collect_dependency_module_public_type_declarations(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            Some(&imported_externs),
            required_types_by_module_path.get(&module_import_path),
            &occupied_root_names,
            &mut type_owners,
            &mut type_declarations,
        )
        .map_err(|error| {
            if report_failure {
                report_package_under_test_type_bridge_error(command_label, package_name, error);
            }
            1
        })?;
    }

    let declarations =
        join_dependency_bridge_sections(&type_declarations.join("\n\n"), &forwarders.join("\n\n"));
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites,
    })
}

fn report_package_under_test_type_bridge_error(
    command_label: &str,
    package_name: &str,
    error: DependencyPublicTypeBridgeError,
) {
    match error {
        DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
            report_package_under_test_symbol_conflict(
                command_label,
                "public type",
                &symbol,
                &owner.package_name,
                package_name,
            );
        }
        DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
            report_package_under_test_local_conflict(
                command_label,
                "public type",
                &symbol,
                "rename the local top-level item or avoid importing a package-under-test public type with the same original symbol name",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ql_project::{BuildTarget, BuildTargetKind, ManifestBuildProfile};

    struct TestDir {
        path: std::path::PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary test directory");
            Self { path }
        }

        fn write(&self, relative: &str, contents: &str) -> std::path::PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create test parent directory");
            }
            fs::write(&path, contents).expect("write test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn package_under_test_bridge_returns_empty_for_unselected_member() {
        let dir = TestDir::new("ql-package-under-test-bridge-unselected");
        let manifest_path = dir.write(
            "app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let source = "use app.add as add\n\nfn main() -> Int { return add(1) }\n";

        let items =
            render_package_under_test_bridge_items("ql test", &[], &manifest_path, source, false)
                .expect("unselected package should not fail bridge rendering");

        assert!(items.declarations.is_empty());
        assert!(items.source_rewrites.is_empty());
    }

    #[test]
    fn package_under_test_bridge_includes_imported_public_function_forwarders() {
        let dir = TestDir::new("ql-package-under-test-bridge-function");
        let manifest_path = dir.write(
            "app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let lib_path = dir.write(
            "app/src/lib.ql",
            "pub fn add(value: Int) -> Int { return value + 1 }\n",
        );
        let workspace_members = vec![WorkspaceBuildTargets {
            member_manifest_path: manifest_path.clone(),
            package_name: "app".to_owned(),
            default_profile: Some(ManifestBuildProfile::Debug),
            targets: vec![BuildTarget {
                kind: BuildTargetKind::Library,
                path: lib_path,
            }],
        }];
        let source = "use app.add as add\n\nfn main() -> Int { return add(1) }\n";

        let items = render_package_under_test_bridge_items(
            "ql test",
            &workspace_members,
            &manifest_path,
            source,
            false,
        )
        .expect("package-under-test public function bridge should render");

        assert!(
            items
                .declarations
                .contains("extern \"c\" fn __ql_bridge_app_add")
        );
        assert!(items.declarations.contains("const add: (Int) -> Int"));
        assert!(items.source_rewrites.is_empty());
    }
}
