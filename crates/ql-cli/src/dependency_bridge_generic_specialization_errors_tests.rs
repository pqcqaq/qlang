use std::path::PathBuf;

use super::*;

#[test]
fn generic_specialization_quiet_mapping_reuses_target_prep_helpers() {
    let interface_failure =
        dependency_generic_specialization_module_load_error_to_target_prep_error(
            DependencyGenericSpecializationModuleLoadError::DependencyInterface {
                dependency_manifest_path: PathBuf::from("workspace/dep/qlang.toml"),
                dependency_package: "dep".to_owned(),
                interface_path: PathBuf::from("workspace/dep/dep.qi"),
                detail: "missing interface".to_owned(),
            },
        );
    let source_read_failure =
        dependency_generic_specialization_module_load_error_to_target_prep_error(
            DependencyGenericSpecializationModuleLoadError::DependencySourceRead {
                dependency_manifest_path: PathBuf::from("workspace/dep/qlang.toml"),
                dependency_package: "dep".to_owned(),
                source_path: PathBuf::from("workspace/dep/src/lib.ql"),
                detail: "access denied".to_owned(),
            },
        );
    let source_parse_failure =
        dependency_generic_specialization_module_load_error_to_target_prep_error(
            DependencyGenericSpecializationModuleLoadError::DependencySourceParse {
                dependency_manifest_path: PathBuf::from("workspace/dep/qlang.toml"),
                dependency_package: "dep".to_owned(),
                source_path: PathBuf::from("workspace/dep/src/lib.ql"),
            },
        );

    match interface_failure.failure_kind {
        crate::build_plan::PrepareProjectTargetBuildFailureKind::DependencyInterface {
            message,
            ..
        } => assert_eq!(
            message,
            "failed to load referenced package interface `workspace/dep/dep.qi`: missing interface"
        ),
        _ => panic!("expected dependency interface failure"),
    }
    match source_read_failure.failure_kind {
        crate::build_plan::PrepareProjectTargetBuildFailureKind::DependencySource {
            message,
            ..
        } => assert_eq!(
            message,
            "failed to access dependency source `workspace/dep/src/lib.ql`: access denied"
        ),
        _ => panic!("expected dependency source read failure"),
    }
    match source_parse_failure.failure_kind {
        crate::build_plan::PrepareProjectTargetBuildFailureKind::DependencySource {
            message,
            ..
        } => assert_eq!(
            message,
            "failed to parse dependency source `workspace/dep/src/lib.ql` while preparing imported generic helper specializations"
        ),
        _ => panic!("expected dependency source parse failure"),
    }
}
