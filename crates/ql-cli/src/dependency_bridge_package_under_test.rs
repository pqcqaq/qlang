use std::path::Path;

use ql_parser::parse_source;
use ql_project::{WorkspaceBuildTargets, load_project_manifest};

use crate::build_plan::report_project_build_dependency_error;
use crate::build_source_rewrites::{
    RenderedDependencyBridgeItems, join_dependency_bridge_sections,
};
use crate::cli_utils::normalize_path;
use crate::dependency_bridge_modules::{
    dependency_generic_specialization_module_refs, dependency_generic_specialization_modules,
    package_under_test_bridge_modules,
};
use crate::dependency_bridge_package_under_test_collection::{
    PackageUnderTestBridgeCollectionError, collect_package_under_test_bridge_items,
};
use crate::dependency_bridge_public_function_errors::report_package_under_test_function_forwarder_error;
use crate::dependency_bridge_public_type_errors::report_package_under_test_type_bridge_error;

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
    let collected = collect_package_under_test_bridge_items(
        package_name,
        manifest_path,
        &root_source_module,
        &bridge_modules,
        &specialization_modules,
    )
    .map_err(|error| {
        if report_failure {
            match error {
                PackageUnderTestBridgeCollectionError::Function(error) => {
                    report_package_under_test_function_forwarder_error(
                        command_label,
                        package_name,
                        error,
                    );
                }
                PackageUnderTestBridgeCollectionError::Type(error) => {
                    report_package_under_test_type_bridge_error(command_label, package_name, error);
                }
            }
        }
        1
    })?;

    let declarations = join_dependency_bridge_sections(
        &collected.type_declarations.join("\n\n"),
        &collected.function_forwarders.join("\n\n"),
    );
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: collected.source_rewrites,
    })
}

#[cfg(test)]
#[path = "dependency_bridge_package_under_test_tests.rs"]
mod tests;
