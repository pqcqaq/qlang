use super::*;

#[test]
fn parse_cli_build_profile_accepts_supported_profiles() {
    assert_eq!(
        parse_cli_build_profile("`ql build`", "debug"),
        Ok(BuildProfile::Debug)
    );
    assert_eq!(
        parse_cli_build_profile("`ql build`", "release"),
        Ok(BuildProfile::Release)
    );
}

#[test]
fn parse_cli_build_profile_rejects_unknown_profiles() {
    assert_eq!(parse_cli_build_profile("`ql build`", "fast"), Err(1));
}

#[test]
fn set_cli_build_profile_rejects_duplicate_selectors() {
    let mut profile = None;
    assert_eq!(
        set_cli_build_profile("`ql run`", &mut profile, BuildProfile::Release),
        Ok(())
    );
    assert_eq!(profile, Some(BuildProfile::Release));
    assert_eq!(
        set_cli_build_profile("`ql run`", &mut profile, BuildProfile::Debug),
        Err(1)
    );
    assert_eq!(profile, Some(BuildProfile::Release));
}
