use std::path::Path;

use ql_project::package_name;

use crate::cli_utils::normalize_path;
use crate::project_interface_reporting::{
    check_package_interface_artifact, report_package_interface_check,
    report_package_interface_failure, report_package_interface_manifest_failure,
};
use crate::project_manifest_paths::workspace_member_manifest_path;
use crate::project_workspace::{
    resolve_selected_workspace_member_manifest, select_workspace_members,
};

mod package_emit;
mod preflight;
mod workspace_check;
mod workspace_emit;

use package_emit::emit_single_package_interface;
use preflight::{
    load_project_emit_interface_manifest, project_emit_interface_labels,
    report_package_interface_check_manifest_failure,
    validate_project_emit_interface_package_selector,
};
use workspace_check::check_workspace_member_interface;
use workspace_emit::emit_workspace_member_interface;

pub(crate) fn project_emit_interface_path(
    path: &Path,
    output: Option<&Path>,
    selected_package_name: Option<&str>,
    changed_only: bool,
    check_only: bool,
) -> Result<(), u8> {
    if check_only && output.is_some() {
        eprintln!("error: `ql project emit-interface --check` does not support `--output`");
        return Err(1);
    }

    let labels = project_emit_interface_labels(output, changed_only, check_only);
    validate_project_emit_interface_package_selector(
        selected_package_name,
        labels.active.as_str(),
    )?;
    let manifest =
        load_project_emit_interface_manifest(path, output, changed_only, check_only, &labels)?;

    if output.is_some() && manifest.workspace.is_some() {
        if let Some(selected_package_name) = selected_package_name {
            let (_, package_manifest) = resolve_selected_workspace_member_manifest(
                &manifest,
                path,
                selected_package_name,
                labels.emit.as_str(),
                "--package",
            )?;
            emit_single_package_interface(
                &package_manifest.manifest_path,
                &package_manifest.manifest_path,
                None,
                output,
                labels.emit.as_str(),
                changed_only,
            )?;
            return Ok(());
        }
    }

    if manifest.package.is_some() {
        if let Some(selected_package_name) = selected_package_name {
            let actual_package_name = package_name(&manifest).map_err(|error| {
                eprintln!("error: {} {error}", labels.active);
                if check_only {
                    report_package_interface_check_manifest_failure(
                        &manifest.manifest_path,
                        changed_only,
                    );
                } else {
                    report_package_interface_manifest_failure(
                        &manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                }
                1
            })?;
            if actual_package_name != selected_package_name {
                eprintln!(
                    "error: {} package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                    labels.active,
                    normalize_path(path)
                );
                return Err(1);
            }
        }
        if check_only {
            let result = match check_package_interface_artifact(
                &manifest,
                labels.check.as_str(),
                changed_only,
            ) {
                Ok(result) => result,
                Err(code) => {
                    report_package_interface_failure(
                        &manifest.manifest_path,
                        None,
                        None,
                        changed_only,
                        None,
                    );
                    return Err(code);
                }
            };
            return report_package_interface_check(
                result,
                None,
                labels.check.as_str(),
                changed_only,
            );
        }
        emit_single_package_interface(
            path,
            &manifest.manifest_path,
            None,
            output,
            labels.emit.as_str(),
            changed_only,
        )?;
        return Ok(());
    }

    if output.is_some() {
        eprintln!("error: `ql project emit-interface --output` only supports package manifests");
        return Err(1);
    }

    let Some(_) = &manifest.workspace else {
        eprintln!("error: `ql project emit-interface` requires `[package]` or `[workspace]`");
        return Err(1);
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let selected_members = select_workspace_members(
        &manifest,
        path,
        selected_package_name,
        labels.active.as_str(),
        "--package",
    )?;
    let mut failing_member_count = 0usize;
    let mut emission_failure_count = 0usize;
    let mut first_failing_member_manifest = None;
    for member in &selected_members {
        let member_manifest_path = workspace_member_manifest_path(&manifest_dir.join(member));
        if check_only {
            if !check_workspace_member_interface(
                &member_manifest_path,
                changed_only,
                labels.check.as_str(),
                &mut first_failing_member_manifest,
            ) {
                failing_member_count += 1;
            }
        } else {
            if !emit_workspace_member_interface(
                &member_manifest_path,
                changed_only,
                labels.emit.as_str(),
                &mut first_failing_member_manifest,
            ) {
                emission_failure_count += 1;
            }
        }
    }

    if check_only && failing_member_count > 0 {
        eprintln!(
            "error: {} found {failing_member_count} failing member(s)",
            labels.check
        );
        if failing_member_count > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    if !check_only && emission_failure_count > 0 {
        eprintln!(
            "error: {} found {emission_failure_count} failing member(s)",
            labels.emit
        );
        if emission_failure_count > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    Ok(())
}
