use std::path::Path;

use ql_project::{BuildTarget, WorkspaceBuildTargets};
use serde_json::json;

use crate::cli_utils::{json_string, normalize_path};

use super::{project_target_display_path, selection::ProjectTargetSelectionFailure};

pub(super) fn render_project_target_members(members: &[WorkspaceBuildTargets], json: bool) {
    if json {
        print!("{}", render_project_targets_json(members));
    } else {
        print_project_target_members(members);
    }
}

fn print_project_target_members(members: &[WorkspaceBuildTargets]) {
    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            println!();
        }
        print_project_target_member(
            member.member_manifest_path.as_path(),
            &member.package_name,
            &member.targets,
        );
    }
}

fn print_project_target_member(manifest_path: &Path, package_name: &str, targets: &[BuildTarget]) {
    println!("manifest: {}", normalize_path(manifest_path));
    println!("package: {package_name}");

    if targets.is_empty() {
        println!("targets: (none found)");
        return;
    }

    println!("targets:");
    for target in targets {
        println!(
            "  - {}: {}",
            target.kind.as_str(),
            project_target_display_path(manifest_path, target.path.as_path())
        );
    }
}

pub(crate) fn render_project_targets_json(members: &[WorkspaceBuildTargets]) -> String {
    let mut rendered = String::new();
    rendered.push_str("{\n");
    rendered.push_str("  \"schema\": \"ql.project.targets.v1\",\n");
    rendered.push_str("  \"members\": [");

    if members.is_empty() {
        rendered.push_str("]\n}\n");
        return rendered;
    }

    rendered.push('\n');
    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            rendered.push_str(",\n");
        }
        rendered.push_str("    {\n");
        rendered.push_str("      \"manifest_path\": ");
        rendered.push_str(&json_string(&normalize_path(
            member.member_manifest_path.as_path(),
        )));
        rendered.push_str(",\n");
        rendered.push_str("      \"package_name\": ");
        rendered.push_str(&json_string(&member.package_name));
        rendered.push_str(",\n");
        rendered.push_str("      \"targets\": [");

        if member.targets.is_empty() {
            rendered.push_str("]\n");
        } else {
            rendered.push('\n');
            for (target_index, target) in member.targets.iter().enumerate() {
                if target_index > 0 {
                    rendered.push_str(",\n");
                }
                rendered.push_str("        {\n");
                rendered.push_str("          \"kind\": ");
                rendered.push_str(&json_string(target.kind.as_str()));
                rendered.push_str(",\n");
                rendered.push_str("          \"path\": ");
                rendered.push_str(&json_string(&project_target_display_path(
                    member.member_manifest_path.as_path(),
                    target.path.as_path(),
                )));
                rendered.push_str("\n        }");
            }
            rendered.push_str("\n      ]\n");
        }

        rendered.push_str("    }");
    }

    rendered.push_str("\n  ]\n}\n");
    rendered
}

pub(super) fn render_project_targets_selection_failure_json(
    failure: &ProjectTargetSelectionFailure,
) -> String {
    let mut rendered = String::new();
    rendered.push_str("{\n");
    rendered.push_str("  \"schema\": \"ql.project.targets.v1\",\n");
    rendered.push_str("  \"members\": [],\n");
    rendered.push_str("  \"failure\": {\n");
    rendered.push_str("    \"kind\": \"selection\",\n");
    rendered.push_str("    \"selection_failure\": {\n");
    rendered.push_str("      \"stage\": ");
    rendered.push_str(&json_string(failure.stage));
    rendered.push_str(",\n");
    rendered.push_str("      \"message\": ");
    rendered.push_str(&json_string(&failure.message));
    rendered.push_str(",\n");
    rendered.push_str("      \"selector\": ");
    rendered.push_str(&json_string(&failure.selector));
    rendered.push_str(",\n");
    rendered.push_str("      \"target_count\": ");
    rendered.push_str(&failure.target_count.to_string());
    rendered.push_str("\n    }\n");
    rendered.push_str("  }\n");
    rendered.push_str("}\n");
    rendered
}

pub(super) fn render_project_targets_preflight_failure_json(
    path: &Path,
    stage: &str,
    message: String,
    manifest_path: Option<&Path>,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.targets.v1",
        "path": normalize_path(path),
        "members": [],
        "failure": {
            "kind": "preflight",
            "preflight_failure": {
                "stage": stage,
                "message": message,
                "manifest_path": manifest_path.map(normalize_path),
            },
        },
    }))
    .expect("project targets preflight failure json should serialize");
    format!("{rendered}\n")
}
