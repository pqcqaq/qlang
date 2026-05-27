use super::*;

fn parse(args: &[&str]) -> Result<CheckCliOptions, u8> {
    parse_check_args(args.iter().map(|arg| arg.to_string()))
}

#[test]
fn parse_check_args_accepts_path_package_json_and_sync_interfaces() {
    let options = parse(&["pkg", "--package", "app", "--json", "--sync-interfaces"])
        .expect("check args should parse");

    assert_eq!(options.path, "pkg");
    assert_eq!(options.package_name.as_deref(), Some("app"));
    assert!(options.json);
    assert!(options.sync_interfaces);
}

#[test]
fn parse_check_args_rejects_missing_path() {
    assert!(parse(&["--json"]).is_err());
}

#[test]
fn parse_check_args_rejects_duplicate_package_selector() {
    assert!(parse(&["pkg", "--package", "app", "--package", "lib"]).is_err());
}
