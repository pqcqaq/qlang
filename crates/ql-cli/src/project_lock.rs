use std::fs;
use std::path::Path;

use ql_project::{load_project_manifest, project_lockfile_path, render_project_lockfile};
use serde_json::{Value as JsonValue, json};

use super::{
    normalize_line_endings, normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
    resolve_project_workspace_member_command_request_root,
};

#[derive(Debug)]
struct ProjectLockJsonReport {
    path: String,
    project_manifest_path: String,
    lockfile_path: String,
    check_only: bool,
    status: &'static str,
    lockfile: JsonValue,
    failure: Option<JsonValue>,
}

impl ProjectLockJsonReport {
    fn new(
        path: &Path,
        manifest: &ql_project::ProjectManifest,
        lockfile_path: &Path,
        check_only: bool,
        rendered_lockfile: &str,
    ) -> Self {
        Self {
            path: normalize_path(path),
            project_manifest_path: normalize_path(&manifest.manifest_path),
            lockfile_path: normalize_path(lockfile_path),
            check_only,
            status: if check_only { "up-to-date" } else { "wrote" },
            lockfile: serde_json::from_str(rendered_lockfile)
                .expect("project lockfile should serialize as valid json"),
            failure: None,
        }
    }

    fn record_failure(
        &mut self,
        kind: &'static str,
        message: String,
        rerun_command: Option<String>,
    ) {
        let mut failure = json!({
            "kind": kind,
            "message": message,
        });
        if let Some(command) = rerun_command {
            failure["rerun_command"] = json!(command);
        }
        self.status = "failed";
        self.failure = Some(failure);
    }

    fn into_json(self) -> String {
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.project.lock.result.v1",
            "path": self.path,
            "project_manifest_path": self.project_manifest_path,
            "lockfile_path": self.lockfile_path,
            "check_only": self.check_only,
            "status": self.status,
            "lockfile": self.lockfile,
            "failure": self.failure,
        }))
        .expect("project lock json report should serialize");
        format!("{rendered}\n")
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ProjectLockCheckStatus {
    UpToDate,
    Stale,
    Missing,
    ReadError(String),
}

fn project_lockfile_check_status(lockfile_path: &Path, expected: &str) -> ProjectLockCheckStatus {
    match fs::read_to_string(lockfile_path) {
        Ok(actual) => {
            if normalize_line_endings(&actual) == normalize_line_endings(expected) {
                ProjectLockCheckStatus::UpToDate
            } else {
                ProjectLockCheckStatus::Stale
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ProjectLockCheckStatus::Missing
        }
        Err(error) => ProjectLockCheckStatus::ReadError(error.to_string()),
    }
}

pub(crate) fn project_lock_path(path: &Path, check_only: bool, json: bool) -> Result<(), u8> {
    let command_label = if check_only {
        "`ql project lock --check`"
    } else {
        "`ql project lock`"
    };

    let request_root = resolve_project_workspace_member_command_request_root(path);
    let manifest = load_project_manifest(request_root.as_deref().unwrap_or(path)).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            eprintln!(
                "error: {command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            );
            report_project_lock_package_context_failure(path, check_only);
        } else if let Some(manifest_path) =
            package_missing_name_manifest_path_from_project_error(&error)
        {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {command_label} {error}");
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;

    let lockfile_path = project_lockfile_path(&manifest);
    let rendered = render_project_lockfile(&manifest).map_err(|error| {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = &error {
            eprintln!(
                "error: {command_label} package source directory `{}` does not exist",
                normalize_path(path)
            );
            eprintln!(
                "hint: rerun `{}` after fixing the package source root",
                format_project_lock_command(&manifest.manifest_path, check_only)
            );
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {command_label} {error}");
            report_project_lock_manifest_failure(manifest_path, check_only);
        } else {
            eprintln!("error: {command_label} {error}");
            eprintln!(
                "note: failing package manifest: {}",
                normalize_path(&manifest.manifest_path)
            );
        }
        1
    })?;

    if json {
        let mut report =
            ProjectLockJsonReport::new(path, &manifest, &lockfile_path, check_only, &rendered);
        let rerun_command = format_project_lock_command(&manifest.manifest_path, false);

        if check_only {
            match project_lockfile_check_status(&lockfile_path, &rendered) {
                ProjectLockCheckStatus::UpToDate => {
                    print!("{}", report.into_json());
                    return Ok(());
                }
                ProjectLockCheckStatus::Stale => {
                    report.record_failure(
                        "stale",
                        format!("lockfile `{}` is stale", normalize_path(&lockfile_path)),
                        Some(rerun_command),
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
                ProjectLockCheckStatus::Missing => {
                    report.record_failure(
                        "missing",
                        format!("lockfile `{}` is missing", normalize_path(&lockfile_path)),
                        Some(rerun_command),
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
                ProjectLockCheckStatus::ReadError(error) => {
                    report.record_failure(
                        "read",
                        format!(
                            "failed to read lockfile `{}`: {error}",
                            normalize_path(&lockfile_path)
                        ),
                        Some(rerun_command),
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
            }
        }

        if let Err(error) = fs::write(&lockfile_path, rendered) {
            report.record_failure(
                "write",
                format!(
                    "failed to write lockfile `{}`: {error}",
                    normalize_path(&lockfile_path)
                ),
                Some(format_project_lock_command(&manifest.manifest_path, false)),
            );
            print!("{}", report.into_json());
            return Err(1);
        }

        print!("{}", report.into_json());
        return Ok(());
    }

    if check_only {
        return check_project_lockfile(&manifest, &lockfile_path, &rendered);
    }

    fs::write(&lockfile_path, rendered).map_err(|error| {
        eprintln!(
            "error: {command_label} failed to write lockfile `{}`: {error}",
            normalize_path(&lockfile_path)
        );
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "hint: rerun `ql project lock {}` after fixing the lockfile output path",
            normalize_path(&manifest.manifest_path)
        );
        1
    })?;

    println!("wrote lockfile: {}", normalize_path(&lockfile_path));
    Ok(())
}

fn check_project_lockfile(
    manifest: &ql_project::ProjectManifest,
    lockfile_path: &Path,
    expected: &str,
) -> Result<(), u8> {
    let normalized_lockfile_path = normalize_path(lockfile_path);
    let rerun_command = format!(
        "ql project lock {}",
        normalize_path(&manifest.manifest_path)
    );

    match project_lockfile_check_status(lockfile_path, expected) {
        ProjectLockCheckStatus::UpToDate => return Ok(()),
        ProjectLockCheckStatus::Stale => {
            eprintln!(
                "error: `ql project lock --check` lockfile `{normalized_lockfile_path}` is stale"
            );
        }
        ProjectLockCheckStatus::Missing => {
            eprintln!(
                "error: `ql project lock --check` lockfile `{normalized_lockfile_path}` is missing"
            );
        }
        ProjectLockCheckStatus::ReadError(error) => {
            eprintln!(
                "error: `ql project lock --check` failed to read lockfile `{normalized_lockfile_path}`: {error}"
            );
        }
    }

    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(&manifest.manifest_path)
    );
    eprintln!("hint: rerun `{rerun_command}` to regenerate `qlang.lock`");
    Err(1)
}

fn format_project_lock_command(manifest_path: &Path, check_only: bool) -> String {
    let manifest_path = normalize_path(manifest_path);
    if check_only {
        format!("ql project lock {manifest_path} --check")
    } else {
        format!("ql project lock {manifest_path}")
    }
}

fn report_project_lock_manifest_failure(manifest_path: &Path, check_only: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = if check_only {
        format!("ql project lock {manifest_path} --check")
    } else {
        format!("ql project lock {manifest_path}")
    };
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_lock_package_context_failure(path: &Path, check_only: bool) {
    let normalized_path = normalize_path(path);
    let rerun_command = if check_only {
        format!("ql project lock {normalized_path} --check")
    } else {
        format!("ql project lock {normalized_path}")
    };
    eprintln!(
        "note: `ql project lock` only writes or checks package/workspace lockfiles for packages or workspace members discoverable from `qlang.toml`"
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ql-cli-{name}-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn lock_check_status_accepts_equivalent_line_endings() {
        let path = unique_temp_file("lock-check-line-endings");
        fs::write(&path, "one\r\ntwo\r\n").expect("write temp lockfile");

        let status = project_lockfile_check_status(&path, "one\ntwo\n");

        let _ = fs::remove_file(&path);
        assert_eq!(status, ProjectLockCheckStatus::UpToDate);
    }

    #[test]
    fn lock_check_status_reports_stale_and_missing_files() {
        let stale_path = unique_temp_file("lock-check-stale");
        let missing_path = unique_temp_file("lock-check-missing");
        fs::write(&stale_path, "old\n").expect("write temp lockfile");

        let stale_status = project_lockfile_check_status(&stale_path, "new\n");
        let missing_status = project_lockfile_check_status(&missing_path, "new\n");

        let _ = fs::remove_file(&stale_path);
        assert_eq!(stale_status, ProjectLockCheckStatus::Stale);
        assert_eq!(missing_status, ProjectLockCheckStatus::Missing);
    }
}
