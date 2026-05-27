use super::*;

#[test]
fn run_build_options_always_requests_executable_output() {
    let options = run_build_options(BuildProfile::Release);

    assert!(matches!(options.emit, BuildEmit::Executable));
    assert!(matches!(options.profile, BuildProfile::Release));
}
