use std::path::Path;

use ql_driver::BuildOptions;

use crate::project_targets::{
    ProjectCommandPathError, ProjectTargetSelector, ResolvedProjectCommandPath,
    report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, resolve_project_command_path,
};
use crate::{
    BuildJsonReport, build_json_emit_interface_failure, build_json_preflight_failure,
    build_project_path, build_single_source_target, build_single_source_target_result,
    emit_built_package_interface, emit_built_package_interface_quiet,
};

pub(crate) fn build_path(
    path: &Path,
    options: &BuildOptions,
    selector: &ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
) -> Result<(), u8> {
    match resolve_project_command_path(path, selector) {
        Ok(ResolvedProjectCommandPath::Project {
            request_root_manifest_path,
            selector,
        }) => {
            return build_project_path(
                path,
                options,
                &selector,
                emit_interface,
                emit_overridden,
                profile_overridden,
                json,
                request_root_manifest_path.as_deref(),
            );
        }
        Ok(ResolvedProjectCommandPath::DirectSource) => {}
        Err(ProjectCommandPathError::SourcePathRejectsSelector) => {
            if json {
                let mut report =
                    BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
                report.record_preflight_failure(build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "selector",
                    "project-context",
                    "direct project source paths do not support target selectors".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_source_path_rejects_target_selector("`ql build`", path, selector);
            }
            return Err(1);
        }
        Err(ProjectCommandPathError::SelectorRequiresProjectContext) => {
            if json {
                let mut report =
                    BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
                report.record_preflight_failure(build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "selector",
                    "project-context",
                    "target selectors require a package or workspace path".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_target_selector_requires_project_context("`ql build`", selector);
            }
            return Err(1);
        }
    }

    if json {
        let mut report =
            BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
        let artifact = match build_single_source_target_result(path, options) {
            Ok(artifact) => artifact,
            Err(error) => {
                report.record_source_failure(path, &error);
                print!("{}", report.into_json());
                return Err(1);
            }
        };
        report.record_source_target(path, &artifact);
        if emit_interface {
            match emit_built_package_interface_quiet(path, path, options, &artifact.path, &[]) {
                Ok(interface_result) => {
                    report.record_interface_result(None, None, true, interface_result);
                }
                Err(error) => {
                    report.record_preflight_failure(build_json_emit_interface_failure(
                        path, None, None, &error,
                    ));
                    print!("{}", report.into_json());
                    return Err(1);
                }
            }
        }
        print!("{}", report.into_json());
        return Ok(());
    }

    let artifact = build_single_source_target(path, options, emit_interface)?;
    if emit_interface {
        emit_built_package_interface(path, path, options, &artifact.path, &[])?;
    }
    Ok(())
}
