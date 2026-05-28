use std::path::{Path, PathBuf};

use ql_driver::{BuildEmit, BuildProfile, default_output_path};

pub(super) fn project_test_output_path(
    manifest_path: &Path,
    test_path: &Path,
    profile: BuildProfile,
) -> PathBuf {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    let tests_root = package_root.join("tests");
    let relative_test = test_path.strip_prefix(&tests_root).unwrap_or(test_path);
    let default_output =
        default_output_path(package_root, test_path, profile, BuildEmit::Executable);
    let file_name = default_output
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("test"));
    let mut output_path = package_root
        .join("target")
        .join("ql")
        .join(profile.dir_name())
        .join("tests");
    if let Some(parent) = relative_test.parent()
        && !parent.as_os_str().is_empty()
    {
        output_path = output_path.join(parent);
    }
    output_path.join(file_name)
}

pub(super) fn package_test_command_path(package_root: &Path, source_path: &Path) -> PathBuf {
    source_path
        .strip_prefix(package_root)
        .unwrap_or(source_path)
        .to_path_buf()
}
