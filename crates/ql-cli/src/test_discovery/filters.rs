use crate::cli_utils::normalize_path;
use crate::test_reporting::{TestTarget, TestTargetKind};

pub(crate) fn filter_test_targets(
    targets: Vec<TestTarget>,
    filter: Option<&str>,
) -> Vec<TestTarget> {
    let Some(filter) = filter else {
        return targets;
    };
    targets
        .into_iter()
        .filter(|target| target.display_path.contains(filter))
        .collect()
}

pub(crate) fn select_test_targets_by_path(
    targets: Vec<TestTarget>,
    target_path: &str,
    package_name: Option<&str>,
) -> Vec<TestTarget> {
    targets
        .into_iter()
        .filter(|target| test_target_matches_path(target, target_path, package_name))
        .collect()
}

fn test_target_matches_path(
    target: &TestTarget,
    target_path: &str,
    package_name: Option<&str>,
) -> bool {
    if target.display_path == target_path {
        return true;
    }

    if let Some(package_name) = package_name
        && let Some(package_relative_path) = target
            .display_path
            .strip_prefix(&format!("packages/{package_name}/"))
        && package_relative_path == target_path
    {
        return true;
    }

    match &target.kind {
        TestTargetKind::Smoke { source_path, .. } | TestTargetKind::Ui { source_path, .. } => {
            normalize_path(source_path) == target_path
        }
    }
}
