use std::path::Path;

use crate::build_plan::PrepareProjectTargetBuildError;
use crate::build_source_rewrites::{
    RenderedDependencyBridgeItems, join_dependency_bridge_sections,
};
use crate::dependency_bridge_externs::{
    render_direct_dependency_extern_declarations,
    render_direct_dependency_extern_declarations_quiet,
};
use crate::dependency_bridge_imports::extend_dependency_bridge_name_requirements;
use crate::dependency_bridge_public_functions::{
    render_direct_dependency_public_function_forwarders,
    render_direct_dependency_public_function_forwarders_quiet,
};
use crate::dependency_bridge_public_methods::{
    render_direct_dependency_public_method_forwarders,
    render_direct_dependency_public_method_forwarders_quiet,
};
use crate::dependency_bridge_public_type_declarations::{
    render_direct_dependency_public_type_declarations,
    render_direct_dependency_public_type_declarations_quiet,
};
use crate::dependency_bridge_public_values::{
    render_direct_dependency_public_value_declarations,
    render_direct_dependency_public_value_declarations_quiet,
};

pub(crate) fn render_direct_dependency_bridge_items(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyBridgeItems, u8> {
    let value_declarations = render_direct_dependency_public_value_declarations(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    let function_forwarders = render_direct_dependency_public_function_forwarders(
        command_label,
        manifest_path,
        source,
        &value_declarations.required_functions_by_module_path,
        report_failure,
    )?;
    let mut required_types_by_module_path = value_declarations.required_types_by_module_path;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &function_forwarders.required_types_by_module_path,
    );
    let method_forwarders = render_direct_dependency_public_method_forwarders(
        command_label,
        manifest_path,
        &required_types_by_module_path,
        report_failure,
    )?;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &method_forwarders.required_types_by_module_path,
    );
    let type_declarations = render_direct_dependency_public_type_declarations(
        command_label,
        manifest_path,
        source,
        &required_types_by_module_path,
        report_failure,
    )?;
    let extern_declarations = render_direct_dependency_extern_declarations(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    let declarations =
        join_dependency_bridge_sections(&type_declarations, &value_declarations.declarations);
    let declarations = join_dependency_bridge_sections(&declarations, &extern_declarations);
    let declarations =
        join_dependency_bridge_sections(&declarations, &function_forwarders.forwarders);
    let declarations =
        join_dependency_bridge_sections(&declarations, &method_forwarders.forwarders);
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: function_forwarders.source_rewrites,
    })
}

pub(crate) fn render_direct_dependency_bridge_items_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<RenderedDependencyBridgeItems, PrepareProjectTargetBuildError> {
    let value_declarations =
        render_direct_dependency_public_value_declarations_quiet(manifest_path, source)?;
    let function_forwarders = render_direct_dependency_public_function_forwarders_quiet(
        manifest_path,
        source,
        &value_declarations.required_functions_by_module_path,
    )?;
    let mut required_types_by_module_path = value_declarations.required_types_by_module_path;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &function_forwarders.required_types_by_module_path,
    );
    let method_forwarders = render_direct_dependency_public_method_forwarders_quiet(
        manifest_path,
        &required_types_by_module_path,
    )?;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &method_forwarders.required_types_by_module_path,
    );
    let type_declarations = render_direct_dependency_public_type_declarations_quiet(
        manifest_path,
        source,
        &required_types_by_module_path,
    )?;
    let extern_declarations =
        render_direct_dependency_extern_declarations_quiet(manifest_path, source)?;
    let declarations =
        join_dependency_bridge_sections(&type_declarations, &value_declarations.declarations);
    let declarations = join_dependency_bridge_sections(&declarations, &extern_declarations);
    let declarations =
        join_dependency_bridge_sections(&declarations, &function_forwarders.forwarders);
    let declarations =
        join_dependency_bridge_sections(&declarations, &method_forwarders.forwarders);
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: function_forwarders.source_rewrites,
    })
}

#[cfg(test)]
#[path = "dependency_bridge_direct_tests.rs"]
mod tests;
