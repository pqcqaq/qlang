use std::path::Path;

use serde_json::{Value as JsonValue, from_str};

use super::*;

#[test]
fn check_json_report_preserves_success_contract() {
    let mut report = CheckJsonReport::new("files", true, None);
    report.record_checked_file(Path::new("src/main.ql"));
    report.record_loaded_interface(Path::new("deps/math/target/math.qlmi"));
    report.record_written_interface(Path::new("target/app.qlmi"));

    let rendered: JsonValue = from_str(&report.into_json()).expect("report should parse as json");

    assert_eq!(rendered["schema"], "ql.check.v1");
    assert_eq!(rendered["scope"], "files");
    assert_eq!(rendered["sync_interfaces"], true);
    assert_eq!(rendered["project_manifest_path"], JsonValue::Null);
    assert_eq!(rendered["status"], "ok");
    assert_eq!(rendered["checked_files"][0], "src/main.ql");
    assert_eq!(
        rendered["loaded_interfaces"][0],
        "deps/math/target/math.qlmi"
    );
    assert_eq!(rendered["written_interfaces"][0], "target/app.qlmi");
    assert_eq!(
        rendered["diagnostic_files"]
            .as_array()
            .expect("diagnostics should be an array")
            .len(),
        0
    );
    assert_eq!(
        rendered["failing_manifests"]
            .as_array()
            .expect("failing manifests should be an array")
            .len(),
        0
    );
}
