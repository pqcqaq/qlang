use super::*;
use ql_driver::{BuildEmit, BuildProfile};

#[test]
fn build_json_report_renders_request_schema() {
    let mut options = BuildOptions {
        emit: BuildEmit::Executable,
        ..BuildOptions::default()
    };
    options.profile = BuildProfile::Release;

    let report = BuildJsonReport::new(Path::new("app/src/main.ql"), None, &options, true, false);
    let json: JsonValue =
        serde_json::from_str(&report.into_json()).expect("report json should parse");

    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "file");
    assert_eq!(json["requested_emit"], "exe");
    assert_eq!(json["requested_profile"], "release");
    assert_eq!(json["profile_overridden"], true);
    assert_eq!(json["status"], "ok");
    assert!(json["built_targets"].as_array().is_some_and(Vec::is_empty));
}
