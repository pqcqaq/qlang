use std::path::Path;

use ql_project::{
    ManifestBuildProfile, ProjectManifest, WorkspaceBuildTargets, discover_package_build_targets,
    package_name,
};

use crate::test_command::TestCommandOptions;

use super::project_errors::report_ql_test_project_preflight_error;

pub(super) fn project_test_build_targets_from_manifest(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ProjectManifest,
    workspace_manifest: Option<&ProjectManifest>,
) -> Result<WorkspaceBuildTargets, u8> {
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: match package_name(manifest) {
            Ok(package_name) => package_name.to_owned(),
            Err(error) => {
                return Err(report_ql_test_project_preflight_error(
                    request_path,
                    command_options,
                    &error,
                    "target-discovery",
                ));
            }
        },
        default_profile: resolved_test_member_default_profile(manifest, workspace_manifest),
        targets: match discover_package_build_targets(manifest) {
            Ok(targets) => targets,
            Err(error) => {
                return Err(report_ql_test_project_preflight_error(
                    request_path,
                    command_options,
                    &error,
                    "target-discovery",
                ));
            }
        },
    })
}

pub(super) fn resolved_test_member_default_profile(
    manifest: &ProjectManifest,
    workspace_manifest: Option<&ProjectManifest>,
) -> Option<ManifestBuildProfile> {
    let workspace_default_profile = workspace_manifest
        .and_then(|manifest| manifest.profile.as_ref().map(|profile| profile.default));
    manifest
        .profile
        .as_ref()
        .map(|profile| profile.default)
        .or(workspace_default_profile)
}
