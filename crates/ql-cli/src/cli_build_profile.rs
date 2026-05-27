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
#[path = "cli_build_profile_tests.rs"]
mod tests;
