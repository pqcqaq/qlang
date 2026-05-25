use std::env;
use std::path::PathBuf;

use crate::project_dependency_edit::{project_add_dependency_path, project_remove_dependency_path};
use crate::project_workspace::resolve_project_workspace_member_package_name;

pub(crate) fn project_add_dependency_cli_path(
    args: &mut impl Iterator<Item = String>,
) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut target_package_name = None;
    let mut package_name = None;
    let mut dependency_path = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!(
                        "error: `ql project add-dependency --package` expects a package name"
                    );
                    return Err(1);
                };
                if target_package_name.is_some() {
                    eprintln!(
                        "error: `ql project add-dependency` received `--package` more than once"
                    );
                    return Err(1);
                }
                target_package_name = Some(value.clone());
            }
            "--name" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project add-dependency --name` expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!(
                        "error: `ql project add-dependency` received `--name` more than once"
                    );
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--path" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql project add-dependency --path` expects a package path");
                    return Err(1);
                };
                if dependency_path.is_some() {
                    eprintln!(
                        "error: `ql project add-dependency` received `--path` more than once"
                    );
                    return Err(1);
                }
                dependency_path = Some(PathBuf::from(value));
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project add-dependency` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project add-dependency` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    if package_name.is_some() && dependency_path.is_some() {
        eprintln!(
            "error: `ql project add-dependency` accepts either `--name <package>` or `--path <file-or-dir>`, not both"
        );
        return Err(1);
    }
    if package_name.is_none() && dependency_path.is_none() {
        eprintln!(
            "error: `ql project add-dependency` requires `--name <package>` or `--path <file-or-dir>`"
        );
        return Err(1);
    };

    project_add_dependency_path(
        &path.unwrap_or_else(default_project_command_path),
        target_package_name.as_deref(),
        package_name.as_deref(),
        dependency_path.as_deref(),
    )
}

pub(crate) fn project_remove_dependency_cli_path(
    args: &mut impl Iterator<Item = String>,
) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut target_package_name = None;
    let mut package_name = None;
    let mut remove_all = false;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!(
                        "error: `ql project remove-dependency --package` expects a package name"
                    );
                    return Err(1);
                };
                if target_package_name.is_some() {
                    eprintln!(
                        "error: `ql project remove-dependency` received `--package` more than once"
                    );
                    return Err(1);
                }
                target_package_name = Some(value.clone());
            }
            "--name" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!(
                        "error: `ql project remove-dependency --name` expects a package name"
                    );
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!(
                        "error: `ql project remove-dependency` received `--name` more than once"
                    );
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--all" => {
                remove_all = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project remove-dependency` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project remove-dependency` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    let path = path.unwrap_or_else(default_project_command_path);
    let package_name = if remove_all {
        if let Some(package_name) = package_name {
            package_name
        } else {
            resolve_project_workspace_member_package_name(
                &path,
                None,
                "`ql project remove-dependency --all`",
            )?
        }
    } else {
        let Some(package_name) = package_name else {
            eprintln!("error: `ql project remove-dependency` requires `--name <package>`");
            return Err(1);
        };
        package_name
    };

    project_remove_dependency_path(
        &path,
        target_package_name.as_deref(),
        &package_name,
        remove_all,
    )
}

fn default_project_command_path() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
