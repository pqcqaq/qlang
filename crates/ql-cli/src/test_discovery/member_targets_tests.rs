use std::path::PathBuf;

use ql_project::{
    ManifestBuildProfile, PackageManifest, ProfileManifest, ProjectManifest, ReferencesManifest,
};

use super::member_targets::resolved_test_member_default_profile;

fn project_manifest(
    manifest_path: &str,
    package_name: Option<&str>,
    default_profile: Option<ManifestBuildProfile>,
) -> ProjectManifest {
    ProjectManifest {
        manifest_path: PathBuf::from(manifest_path),
        package: package_name.map(|name| PackageManifest {
            name: name.to_owned(),
        }),
        workspace: None,
        references: ReferencesManifest::default(),
        profile: default_profile.map(|default| ProfileManifest { default }),
        lib: None,
        bins: Vec::new(),
    }
}

#[test]
fn test_member_default_profile_inherits_workspace_default() {
    let workspace = project_manifest(
        "workspace/qlang.toml",
        None,
        Some(ManifestBuildProfile::Release),
    );
    let member = project_manifest("workspace/packages/app/qlang.toml", Some("app"), None);

    assert_eq!(
        resolved_test_member_default_profile(&member, Some(&workspace)),
        Some(ManifestBuildProfile::Release)
    );
}

#[test]
fn test_member_default_profile_prefers_member_default() {
    let workspace = project_manifest(
        "workspace/qlang.toml",
        None,
        Some(ManifestBuildProfile::Release),
    );
    let member = project_manifest(
        "workspace/packages/app/qlang.toml",
        Some("app"),
        Some(ManifestBuildProfile::Debug),
    );

    assert_eq!(
        resolved_test_member_default_profile(&member, Some(&workspace)),
        Some(ManifestBuildProfile::Debug)
    );
}
