use std::env;
use std::path::Path;

use ql_driver::BuildOptions;
use ql_project::WorkspaceBuildTargets;

use crate::build_outputs::apply_manifest_default_profile;
use crate::cli_utils::normalize_path;
use crate::project_targets::display_relative_to_root;
use crate::test_reporting::{TestTarget, TestTargetKind};

use super::paths::{package_test_command_path, project_test_output_path};
use super::ui::is_project_ui_test;

pub(super) fn direct_test_target(path: &Path, options: &BuildOptions) -> Result<TestTarget, u8> {
    let working_directory = env::current_dir().map_err(|error| {
        eprintln!("error: failed to determine the current directory for `ql test`: {error}");
        1
    })?;
    Ok(TestTarget {
        display_path: normalize_path(path),
        kind: TestTargetKind::Smoke {
            source_path: path.to_path_buf(),
            working_directory,
            build_options: options.clone(),
            package_manifest_path: None,
        },
    })
}

pub(super) fn project_test_target(
    request_root: &Path,
    member: &WorkspaceBuildTargets,
    package_root: &Path,
    file: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
) -> TestTarget {
    let display_path = display_relative_to_root(request_root, file);
    if is_project_ui_test(package_root, file) {
        return TestTarget {
            display_path,
            kind: TestTargetKind::Ui {
                source_path: file.to_path_buf(),
                diagnostic_path: package_test_command_path(package_root, file),
                snapshot_path: file.with_extension("stderr"),
            },
        };
    }

    let mut build_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    build_options.output = Some(project_test_output_path(
        &member.member_manifest_path,
        file,
        build_options.profile,
    ));
    TestTarget {
        display_path,
        kind: TestTargetKind::Smoke {
            source_path: file.to_path_buf(),
            working_directory: package_root.to_path_buf(),
            build_options,
            package_manifest_path: Some(member.member_manifest_path.clone()),
        },
    }
}
