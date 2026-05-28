use std::path::{Path, PathBuf};

use ql_driver::BuildProfile;

use super::paths::{package_test_command_path, project_test_output_path};

#[test]
fn project_test_output_path_preserves_nested_test_layout() {
    let output_path = project_test_output_path(
        Path::new("pkg/qlang.toml"),
        Path::new("pkg/tests/integration/math.ql"),
        BuildProfile::Release,
    );
    let executable_name = if cfg!(windows) { "math.exe" } else { "math" };

    assert_eq!(
        output_path,
        PathBuf::from("pkg")
            .join("target")
            .join("ql")
            .join("release")
            .join("tests")
            .join("integration")
            .join(executable_name)
    );
}

#[test]
fn package_test_command_path_is_package_relative() {
    assert_eq!(
        package_test_command_path(Path::new("pkg"), Path::new("pkg/tests/ui/basic.ql")),
        PathBuf::from("tests").join("ui").join("basic.ql")
    );
}
