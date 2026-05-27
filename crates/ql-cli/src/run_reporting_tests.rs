use std::path::Path;

use ql_driver::{BuildEmit, BuildOptions, BuildProfile};

use super::*;

#[test]
fn run_json_report_renders_execution_schema() {
    let mut options = BuildOptions {
        emit: BuildEmit::Executable,
        ..BuildOptions::default()
    };
    options.profile = BuildProfile::Release;
    let mut report = RunJsonReport::new(
        Path::new("app/src/main.ql"),
        None,
        &options,
        true,
        &["--demo".to_owned()],
    );

    report.record_execution(0, "ok\n", "");
    let json = report.into_json();

    assert!(json.contains("\"schema\": \"ql.run.v1\""));
    assert!(json.contains("\"requested_profile\": \"release\""));
    assert!(json.contains("\"program_args\": ["));
    assert!(json.contains("\"stdout\": \"ok\\n\""));
}
