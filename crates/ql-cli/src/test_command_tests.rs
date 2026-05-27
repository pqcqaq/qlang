use ql_driver::BuildProfile;

use super::*;

#[test]
fn parse_test_args_records_profile_selectors_and_filters() {
    let mut args = vec![
        "pkg".to_owned(),
        "--profile".to_owned(),
        "release".to_owned(),
        "--package".to_owned(),
        "app".to_owned(),
        "--target".to_owned(),
        "tests/./api/../smoke.ql".to_owned(),
        "--filter".to_owned(),
        "smoke".to_owned(),
        "--list".to_owned(),
        "--json".to_owned(),
    ]
    .into_iter();

    let parsed = parse_test_args(&mut args).expect("valid test args");

    assert_eq!(parsed.path, "pkg");
    assert_eq!(parsed.command_options.profile, BuildProfile::Release);
    assert!(parsed.command_options.profile_overridden);
    assert!(parsed.command_options.list_only);
    assert!(parsed.command_options.json);
    assert_eq!(parsed.command_options.package_name.as_deref(), Some("app"));
    assert_eq!(parsed.command_options.filter.as_deref(), Some("smoke"));
    assert_eq!(
        parsed.command_options.target_path.as_deref(),
        Some("tests/smoke.ql")
    );
}

#[test]
fn parse_test_args_rejects_duplicate_target_selectors() {
    let mut args = vec![
        "pkg".to_owned(),
        "--target".to_owned(),
        "tests/a.ql".to_owned(),
        "--target".to_owned(),
        "tests/b.ql".to_owned(),
    ]
    .into_iter();

    assert_eq!(parse_test_args(&mut args).map(|_| ()), Err(1));
}
