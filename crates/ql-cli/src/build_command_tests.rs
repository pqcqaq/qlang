use super::*;

fn parse(args: &[&str]) -> Result<BuildCliOptions, u8> {
    parse_build_args(&mut args.iter().map(|arg| arg.to_string()))
}

#[test]
fn parse_build_args_accepts_json_list_emit_interface_and_output() {
    let options = parse(&[
        "--json",
        "--list",
        "--emit-interface",
        "--emit",
        "obj",
        "--output",
        "out/app.o",
    ])
    .expect("build args should parse");

    assert!(options.json);
    assert!(options.list);
    assert!(options.emit_interface);
    assert!(options.emit_overridden);
    assert!(matches!(options.build_options.emit, BuildEmit::Object));
    assert_eq!(
        options.build_options.output.as_deref(),
        Some(Path::new("out/app.o"))
    );
}

#[test]
fn parse_build_args_rejects_conflicting_profile_options() {
    assert!(parse(&["--release", "--profile", "debug"]).is_err());
}
