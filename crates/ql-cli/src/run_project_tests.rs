use ql_driver::{BuildEmit, BuildProfile};

use super::*;

#[test]
fn run_target_build_options_applies_default_output_path() {
    let options = BuildOptions {
        emit: BuildEmit::Executable,
        profile: BuildProfile::Debug,
        ..BuildOptions::default()
    };
    let runnable = RunnableProjectTarget {
        member_manifest_path: Path::new("workspace/app/qlang.toml").to_path_buf(),
        package_name: "app".to_owned(),
        default_profile: None,
        target: ql_project::BuildTarget {
            kind: ql_project::BuildTargetKind::Binary,
            path: Path::new("workspace/app/src/bin/tool.ql").to_path_buf(),
        },
    };

    let target_options = run_target_build_options(&options, false, &runnable);

    assert_eq!(
        target_options.output.as_deref(),
        Some(Path::new("workspace/app/target/ql/debug/bin/tool.exe"))
    );
}
