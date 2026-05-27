use super::*;

#[test]
fn target_prep_dependency_interface_failure_preserves_dependency_context() {
    let error = target_prep_dependency_interface_failure(
        Path::new("workspace/dep/qlang.toml"),
        "dep",
        Path::new("workspace/dep/dep.qi"),
        "missing interface",
    );

    let PrepareProjectTargetBuildFailureKind::DependencyInterface {
        dependency_manifest_path,
        dependency_package,
        interface_path,
        message,
    } = error.failure_kind
    else {
        panic!("expected dependency interface target-prep failure");
    };

    assert_eq!(
        dependency_manifest_path,
        PathBuf::from("workspace/dep/qlang.toml")
    );
    assert_eq!(dependency_package, "dep");
    assert_eq!(interface_path, PathBuf::from("workspace/dep/dep.qi"));
    assert_eq!(
        message,
        "failed to load referenced package interface `workspace/dep/dep.qi`: missing interface"
    );
}

#[test]
fn target_prep_dependency_source_failures_preserve_message_contracts() {
    let read_error = target_prep_dependency_source_read_failure(
        Path::new("workspace/dep/qlang.toml"),
        "dep",
        Path::new("workspace/dep/src/lib.ql"),
        "access denied",
    );
    let parse_error = target_prep_dependency_source_parse_failure(
        Path::new("workspace/dep/qlang.toml"),
        "dep",
        Path::new("workspace/dep/src/lib.ql"),
        "public value bridges",
    );

    let PrepareProjectTargetBuildFailureKind::DependencySource {
        dependency_manifest_path,
        dependency_package,
        source_path,
        message,
    } = read_error.failure_kind
    else {
        panic!("expected dependency source read target-prep failure");
    };
    assert_eq!(
        dependency_manifest_path,
        PathBuf::from("workspace/dep/qlang.toml")
    );
    assert_eq!(dependency_package, "dep");
    assert_eq!(source_path, PathBuf::from("workspace/dep/src/lib.ql"));
    assert_eq!(
        message,
        "failed to access dependency source `workspace/dep/src/lib.ql`: access denied"
    );

    let PrepareProjectTargetBuildFailureKind::DependencySource { message, .. } =
        parse_error.failure_kind
    else {
        panic!("expected dependency source parse target-prep failure");
    };
    assert_eq!(
        message,
        "failed to parse dependency source `workspace/dep/src/lib.ql` while preparing public value bridges"
    );
}
