use std::path::{Path, PathBuf};

pub(crate) fn reference_manifest_path(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
) -> PathBuf {
    let manifest_dir = owner_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let reference_path = manifest_dir.join(reference);
    if reference_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return reference_path;
    }
    reference_path.join("qlang.toml")
}

pub(crate) fn workspace_member_manifest_path(path: &Path) -> PathBuf {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return path.to_path_buf();
    }

    path.join("qlang.toml")
}

pub(crate) fn record_reference_failure_manifest(slot: &mut Option<PathBuf>, path: PathBuf) {
    if slot.is_none() {
        *slot = Some(path);
    }
}
