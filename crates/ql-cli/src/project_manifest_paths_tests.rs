use super::*;

fn manifest_at(path: &str) -> ql_project::ProjectManifest {
    ql_project::ProjectManifest {
        manifest_path: PathBuf::from(path),
        package: None,
        workspace: None,
        references: ql_project::ReferencesManifest::default(),
        profile: None,
        lib: None,
        bins: Vec::new(),
    }
}

#[test]
fn reference_manifest_path_accepts_package_directories_and_manifest_files() {
    let manifest = manifest_at("workspace/app/qlang.toml");

    assert_eq!(
        reference_manifest_path(&manifest, "../dep"),
        PathBuf::from("workspace/app/../dep/qlang.toml")
    );
    assert_eq!(
        reference_manifest_path(&manifest, "../dep/qlang.toml"),
        PathBuf::from("workspace/app/../dep/qlang.toml")
    );
}

#[test]
fn workspace_member_manifest_path_accepts_direct_manifest_files() {
    assert_eq!(
        workspace_member_manifest_path(Path::new("workspace/app")),
        PathBuf::from("workspace/app/qlang.toml")
    );
    assert_eq!(
        workspace_member_manifest_path(Path::new("workspace/app/qlang.toml")),
        PathBuf::from("workspace/app/qlang.toml")
    );
}

#[test]
fn record_reference_failure_manifest_keeps_first_failure() {
    let mut slot = None;
    record_reference_failure_manifest(&mut slot, PathBuf::from("first/qlang.toml"));
    record_reference_failure_manifest(&mut slot, PathBuf::from("second/qlang.toml"));
    assert_eq!(slot, Some(PathBuf::from("first/qlang.toml")));
}
