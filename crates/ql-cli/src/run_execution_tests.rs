use std::path::Path;

use super::*;

#[test]
fn executable_capture_reports_spawn_failures_with_normalized_path() {
    let missing = Path::new("target/ql/missing-run-target");
    let error = run_built_executable_capture(missing, &[])
        .expect_err("missing executable should fail before capture");

    assert!(error.contains("failed to run built executable `target/ql/missing-run-target`"));
}
