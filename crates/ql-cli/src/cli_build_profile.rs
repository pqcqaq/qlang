use ql_driver::BuildProfile;

pub(crate) fn parse_cli_build_profile(
    command_label: &str,
    value: &str,
) -> Result<BuildProfile, u8> {
    match value {
        "debug" => Ok(BuildProfile::Debug),
        "release" => Ok(BuildProfile::Release),
        other => {
            eprintln!("error: {command_label} unsupported profile `{other}`");
            eprintln!("hint: supported profiles are `debug` and `release`");
            Err(1)
        }
    }
}

pub(crate) fn set_cli_build_profile(
    command_label: &str,
    current: &mut Option<BuildProfile>,
    profile: BuildProfile,
) -> Result<(), u8> {
    if current.is_some() {
        eprintln!("error: {command_label} received multiple profile selectors");
        return Err(1);
    }
    *current = Some(profile);
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
