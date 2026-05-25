use std::env;
use std::path::PathBuf;

use crate::project_lock::project_lock_path;
use crate::project_members::project_add_binary_target_path;

pub(crate) fn project_target_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let Some(target_subcommand) = args.next() else {
        eprintln!("error: `ql project target` expects a subcommand");
        return Err(1);
    };

    match target_subcommand.as_str() {
        "add" => project_target_add_cli_path(args),
        other => {
            eprintln!("error: unknown `ql project target` subcommand `{other}`");
            Err(1)
        }
    }
}

pub(crate) fn project_lock_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut check_only = false;
    let mut json = false;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--check" => {
                check_only = true;
            }
            "--json" => {
                json = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project lock` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project lock` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    project_lock_path(
        &path.unwrap_or_else(default_project_command_path),
        check_only,
        json,
    )
}

fn project_target_add_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut target_package_name = None;
    let mut binary_name = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project target add --package` expects a package name");
                    return Err(1);
                };
                if target_package_name.is_some() {
                    eprintln!("error: `ql project target add` received `--package` more than once");
                    return Err(1);
                }
                target_package_name = Some(value.clone());
            }
            "--bin" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project target add --bin` expects a target name");
                    return Err(1);
                };
                if binary_name.is_some() {
                    eprintln!("error: `ql project target add` received `--bin` more than once");
                    return Err(1);
                }
                binary_name = Some(value.clone());
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project target add` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project target add` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    let Some(binary_name) = binary_name else {
        eprintln!("error: `ql project target add` expects `--bin <name>`");
        return Err(1);
    };
    project_add_binary_target_path(
        &path.unwrap_or_else(default_project_command_path),
        target_package_name.as_deref(),
        &binary_name,
    )
}

fn default_project_command_path() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
