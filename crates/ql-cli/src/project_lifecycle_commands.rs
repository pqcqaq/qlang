use std::env;
use std::path::PathBuf;

use crate::project_init::project_init_path;
use crate::project_members::{project_add_existing_path, project_add_path, project_remove_path};

pub(crate) fn project_init_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut workspace = false;
    let mut package_name = None;
    let mut stdlib_path = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--workspace" => {
                workspace = true;
            }
            "--name" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project init --name` expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: `ql project init` received `--name` more than once");
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--stdlib" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project init --stdlib` expects a stdlib workspace path");
                    return Err(1);
                };
                if stdlib_path.is_some() {
                    eprintln!("error: `ql project init` received `--stdlib` more than once");
                    return Err(1);
                }
                stdlib_path = Some(PathBuf::from(value));
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project init` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project init` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    project_init_path(
        &path.unwrap_or_else(default_project_command_path),
        workspace,
        package_name.as_deref(),
        stdlib_path.as_deref(),
    )
}

pub(crate) fn project_add_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut package_name = None;
    let mut existing_path = None;
    let mut dependencies = Vec::new();
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--name" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project add --name` expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: `ql project add` received `--name` more than once");
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--existing" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project add --existing` expects a file or directory");
                    return Err(1);
                };
                if existing_path.is_some() {
                    eprintln!("error: `ql project add` received `--existing` more than once");
                    return Err(1);
                }
                existing_path = Some(PathBuf::from(value));
            }
            "--dependency" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project add --dependency` expects a package name");
                    return Err(1);
                };
                dependencies.push(value.clone());
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project add` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project add` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    let path = path.unwrap_or_else(default_project_command_path);
    if let Some(existing_path) = existing_path {
        if package_name.is_some() {
            eprintln!(
                "error: `ql project add --existing` does not accept `--name`; package name comes from the existing manifest"
            );
            return Err(1);
        }
        if !dependencies.is_empty() {
            eprintln!(
                "error: `ql project add --existing` does not accept `--dependency`; existing manifests keep their current local dependencies"
            );
            return Err(1);
        }
        project_add_existing_path(&path, &existing_path)
    } else {
        let Some(package_name) = package_name else {
            eprintln!("error: `ql project add` requires `--name <package>`");
            return Err(1);
        };
        project_add_path(&path, &package_name, &dependencies)
    }
}

pub(crate) fn project_remove_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut package_name = None;
    let mut cascade = false;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--name" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project remove --name` expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: `ql project remove` received `--name` more than once");
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--cascade" => {
                cascade = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project remove` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project remove` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    let Some(package_name) = package_name else {
        eprintln!("error: `ql project remove` requires `--name <package>`");
        return Err(1);
    };

    project_remove_path(
        &path.unwrap_or_else(default_project_command_path),
        &package_name,
        cascade,
    )
}

fn default_project_command_path() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
