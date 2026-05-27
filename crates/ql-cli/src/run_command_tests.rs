use super::*;

fn parse(args: &[&str]) -> Result<RunCliOptions, u8> {
    parse_run_args(&mut args.iter().map(|arg| arg.to_string()))
}

#[test]
fn parse_run_args_accepts_json_list_profile_and_program_args() {
    let options = parse(&[
        "app",
        "--json",
        "--list",
        "--profile",
        "release",
        "--",
        "--user-flag",
    ])
    .expect("run args should parse");

    assert_eq!(options.path, "app");
    assert!(options.json);
    assert!(options.list);
    assert!(options.profile_overridden);
    assert!(matches!(options.profile, BuildProfile::Release));
    assert_eq!(options.program_args, vec!["--user-flag".to_owned()]);
}
