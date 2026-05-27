use std::path::Path;

use ql_driver::ToolchainError;

use super::toolchain_targets_build_output_path;

#[test]
fn toolchain_output_path_detection_accepts_codegen_temp_artifact() {
    let output_path = Path::new("C:/tmp/workspace/app/build/app.obj");
    let error = ToolchainError::InvocationFailed {
        program: "clang".to_owned(),
        status: Some(9),
        stderr: "unable to open output file 'C:/tmp/workspace/app/build/app.123.codegen.obj': Permission denied".to_owned(),
    };

    assert!(toolchain_targets_build_output_path(&error, output_path));
}

#[test]
fn toolchain_output_path_detection_ignores_non_open_failures() {
    let output_path = Path::new("C:/tmp/workspace/app/build/app.obj");
    let error = ToolchainError::InvocationFailed {
        program: "clang".to_owned(),
        status: Some(1),
        stderr: "undefined symbol while linking C:/tmp/workspace/app/build/app.123.codegen.obj"
            .to_owned(),
    };

    assert!(!toolchain_targets_build_output_path(&error, output_path));
}
