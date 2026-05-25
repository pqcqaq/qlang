use std::path::Path;

use super::check_path;

pub(crate) fn check_cli_path(args: impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_check_args(args)?;
    check_path(
        Path::new(&options.path),
        options.sync_interfaces,
        options.json,
        options.package_name.as_deref(),
    )
}

struct CheckCliOptions {
    path: String,
    sync_interfaces: bool,
    json: bool,
    package_name: Option<String>,
}

fn parse_check_args(args: impl Iterator<Item = String>) -> Result<CheckCliOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut sync_interfaces = false;
    let mut json = false;
    let mut package_name = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--sync-interfaces" => {
                sync_interfaces = true;
            }
            "--json" => {
                json = true;
            }
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql check --package` expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: `ql check` received multiple `--package` selectors");
                    return Err(1);
                }
                package_name = Some(value.to_owned());
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql check` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql check` argument `{other}`");
                    return Err(1);
                }
                path = Some(other.to_owned());
            }
        }

        index += 1;
    }

    let Some(path) = path else {
        eprintln!("error: `ql check` expects a file or directory path");
        return Err(1);
    };

    Ok(CheckCliOptions {
        path,
        sync_interfaces,
        json,
        package_name,
    })
}
