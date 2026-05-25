use std::path::Path;

use ql_driver::BuildProfile;

use super::{
    TestCommandOptions, normalize_path, parse_cli_build_profile, set_cli_build_profile, test_path,
};

pub(crate) fn test_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_test_args(args)?;
    test_path(Path::new(&options.path), &options.command_options)
}

struct TestCliOptions {
    path: String,
    command_options: TestCommandOptions,
}

fn parse_test_args(args: &mut impl Iterator<Item = String>) -> Result<TestCliOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut command_options = TestCommandOptions::default();
    let mut profile_override = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--release" => {
                set_cli_build_profile("`ql test`", &mut profile_override, BuildProfile::Release)?;
            }
            "--profile" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --profile` expects `debug` or `release`");
                    return Err(1);
                };
                let parsed = parse_cli_build_profile("`ql test`", value)?;
                set_cli_build_profile("`ql test`", &mut profile_override, parsed)?;
            }
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --package` expects a package name");
                    return Err(1);
                };
                if command_options.package_name.is_some() {
                    eprintln!("error: `ql test` received multiple `--package` selectors");
                    return Err(1);
                }
                command_options.package_name = Some(value.to_owned());
            }
            "--list" => {
                command_options.list_only = true;
            }
            "--json" => {
                command_options.json = true;
            }
            "--filter" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --filter` expects a substring");
                    return Err(1);
                };
                command_options.filter = Some(value.to_owned());
            }
            "--target" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --target` expects a test path");
                    return Err(1);
                };
                if command_options.target_path.is_some() {
                    eprintln!("error: `ql test` received multiple `--target` selectors");
                    return Err(1);
                }
                command_options.target_path = Some(normalize_path(Path::new(value)));
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql test` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql test` argument `{other}`");
                    return Err(1);
                }
                path = Some(other.to_owned());
            }
        }

        index += 1;
    }

    let Some(path) = path else {
        eprintln!("error: `ql test` expects a file or directory path");
        return Err(1);
    };

    command_options.profile = profile_override.unwrap_or_default();
    command_options.profile_overridden = profile_override.is_some();

    Ok(TestCliOptions {
        path,
        command_options,
    })
}
