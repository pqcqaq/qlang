use std::path::Path;

use ql_project::{BuildTarget, BuildTargetKind, WorkspaceBuildTargets};

use super::{
    json_string, load_workspace_build_targets_for_command_from_request_root, normalize_path,
    resolve_project_workspace_member_command_request_root,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectTargetSelector {
    pub(crate) package_name: Option<String>,
    pub(crate) target: Option<ProjectTargetSelectorKind>,
}

#[derive(Clone, Debug)]
pub(crate) enum ProjectTargetSelectorKind {
    Library,
    Binary(String),
    DisplayPath(String),
}

impl ProjectTargetSelector {
    pub(crate) fn is_active(&self) -> bool {
        self.package_name.is_some() || self.target.is_some()
    }

    pub(crate) fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(package_name) = self.package_name.as_ref() {
            parts.push(format!("package `{package_name}`"));
        }
        if let Some(target) = self.target.as_ref() {
            parts.push(match target {
                ProjectTargetSelectorKind::Library => "library target".to_owned(),
                ProjectTargetSelectorKind::Binary(name) => format!("binary `{name}`"),
                ProjectTargetSelectorKind::DisplayPath(path) => format!("target `{path}`"),
            });
        }
        parts.join(", ")
    }

    pub(crate) fn matches(
        &self,
        manifest_path: &Path,
        package_name: &str,
        target: &BuildTarget,
    ) -> bool {
        if self
            .package_name
            .as_ref()
            .is_some_and(|expected| expected != package_name)
        {
            return false;
        }

        match self.target.as_ref() {
            None => true,
            Some(ProjectTargetSelectorKind::Library) => target.kind == BuildTargetKind::Library,
            Some(ProjectTargetSelectorKind::Binary(name)) => {
                target.kind == BuildTargetKind::Binary
                    && target
                        .path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem == name)
            }
            Some(ProjectTargetSelectorKind::DisplayPath(path)) => {
                project_target_display_path(manifest_path, target.path.as_path()) == *path
            }
        }
    }
}

pub(crate) fn parse_project_target_selector_option(
    command_label: &str,
    remaining: &[String],
    index: &mut usize,
    selector: &mut ProjectTargetSelector,
) -> Result<bool, u8> {
    match remaining[*index].as_str() {
        "--package" => {
            *index += 1;
            let Some(value) = remaining.get(*index) else {
                eprintln!("error: {command_label} --package expects a package name");
                return Err(1);
            };
            if selector.package_name.is_some() {
                eprintln!("error: {command_label} received multiple `--package` selectors");
                return Err(1);
            }
            selector.package_name = Some(value.to_owned());
            Ok(true)
        }
        "--lib" => {
            set_project_target_selector_kind(
                command_label,
                selector,
                ProjectTargetSelectorKind::Library,
            )?;
            Ok(true)
        }
        "--bin" => {
            *index += 1;
            let Some(value) = remaining.get(*index) else {
                eprintln!("error: {command_label} --bin expects a target name");
                return Err(1);
            };
            set_project_target_selector_kind(
                command_label,
                selector,
                ProjectTargetSelectorKind::Binary(value.to_owned()),
            )?;
            Ok(true)
        }
        "--target" => {
            *index += 1;
            let Some(value) = remaining.get(*index) else {
                eprintln!("error: {command_label} --target expects a target path");
                return Err(1);
            };
            set_project_target_selector_kind(
                command_label,
                selector,
                ProjectTargetSelectorKind::DisplayPath(normalize_path(Path::new(value))),
            )?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn set_project_target_selector_kind(
    command_label: &str,
    selector: &mut ProjectTargetSelector,
    kind: ProjectTargetSelectorKind,
) -> Result<(), u8> {
    if selector.target.is_some() {
        eprintln!(
            "error: {command_label} does not support combining `--lib`, `--bin`, and `--target`"
        );
        return Err(1);
    }
    selector.target = Some(kind);
    Ok(())
}

pub(crate) fn report_project_target_selector_requires_project_context(
    command_label: &str,
    selector: &ProjectTargetSelector,
) {
    eprintln!("error: {command_label} target selectors require a package or workspace path");
    eprintln!("note: selector: {}", selector.describe());
}

pub(crate) fn report_project_source_path_rejects_target_selector(
    command_label: &str,
    path: &Path,
    selector: &ProjectTargetSelector,
) {
    eprintln!(
        "error: {command_label} does not support combining a direct project source path with target selectors"
    );
    eprintln!("note: source path: {}", normalize_path(path));
    eprintln!("note: selector: {}", selector.describe());
}

pub(crate) fn select_workspace_build_targets(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
    command_label: &str,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    if !selector.is_active() {
        return Ok(members.to_vec());
    }

    let mut selected = Vec::new();
    for member in members {
        let targets = member
            .targets
            .iter()
            .filter(|target| {
                selector.matches(
                    member.member_manifest_path.as_path(),
                    &member.package_name,
                    target,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            selected.push(WorkspaceBuildTargets {
                member_manifest_path: member.member_manifest_path.clone(),
                package_name: member.package_name.clone(),
                default_profile: member.default_profile,
                targets,
            });
        }
    }

    if selected
        .iter()
        .map(|member| member.targets.len())
        .sum::<usize>()
        == 0
    {
        let normalized_path = normalize_path(path);
        eprintln!(
            "error: {command_label} target selector matched no {target_label} under `{normalized_path}`"
        );
        eprintln!("note: selector: {}", selector.describe());
        eprintln!(
            "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }

    Ok(selected)
}

fn filter_workspace_build_targets(
    members: &[WorkspaceBuildTargets],
    keep_empty_members: bool,
    predicate: impl Fn(&BuildTarget) -> bool,
) -> Vec<WorkspaceBuildTargets> {
    members
        .iter()
        .filter_map(|member| {
            let targets = member
                .targets
                .iter()
                .filter(|target| predicate(target))
                .cloned()
                .collect::<Vec<_>>();
            if targets.is_empty() && !keep_empty_members {
                return None;
            }
            Some(WorkspaceBuildTargets {
                member_manifest_path: member.member_manifest_path.clone(),
                package_name: member.package_name.clone(),
                default_profile: member.default_profile,
                targets,
            })
        })
        .collect()
}

fn load_project_target_members_for_workspace_member_path(
    path: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    load_workspace_build_targets_for_command_from_request_root(
        path,
        request_root.as_deref().unwrap_or(path),
        command_label,
    )
}

pub(crate) fn project_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members =
        load_project_target_members_for_workspace_member_path(path, "`ql project targets`")?;
    let members = select_workspace_build_targets(
        path,
        &members,
        selector,
        "`ql project targets`",
        "build targets",
    )?;
    render_project_target_members(&members, json);
    Ok(())
}

pub(crate) fn list_build_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members = load_project_target_members_for_workspace_member_path(path, "`ql build --list`")?;
    let members = select_workspace_build_targets(
        path,
        &members,
        selector,
        "`ql build --list`",
        "build targets",
    )?;
    render_project_target_members(&members, json);
    Ok(())
}

pub(crate) fn list_runnable_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members = load_project_target_members_for_workspace_member_path(path, "`ql run --list`")?;
    let selected = if selector.is_active() {
        select_workspace_build_targets(
            path,
            &members,
            selector,
            "`ql run --list`",
            "build targets",
        )?
    } else {
        members
    };
    let runnable_members =
        filter_workspace_build_targets(&selected, !selector.is_active(), |target| {
            is_runnable_project_target(target.kind)
        });
    if selector.is_active()
        && runnable_members
            .iter()
            .map(|member| member.targets.len())
            .sum::<usize>()
            == 0
    {
        let normalized_path = normalize_path(path);
        eprintln!(
            "error: `ql run --list` target selector matched no runnable build targets under `{normalized_path}`"
        );
        eprintln!("note: selector: {}", selector.describe());
        eprintln!(
            "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }
    render_project_target_members(&runnable_members, json);
    Ok(())
}

pub(crate) fn is_runnable_project_target(kind: BuildTargetKind) -> bool {
    matches!(kind, BuildTargetKind::Binary | BuildTargetKind::Source)
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

fn render_project_target_members(members: &[WorkspaceBuildTargets], json: bool) {
    if json {
        print!("{}", render_project_targets_json(members));
    } else {
        print_project_target_members(members);
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

pub(crate) fn project_target_display_path(manifest_path: &Path, target_path: &Path) -> String {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    if let Ok(relative) = target_path.strip_prefix(package_root) {
        normalize_path(relative)
    } else {
        normalize_path(target_path)
    }
}

fn render_project_targets_json(members: &[WorkspaceBuildTargets]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn member_with_targets(targets: Vec<BuildTarget>) -> WorkspaceBuildTargets {
        WorkspaceBuildTargets {
            member_manifest_path: PathBuf::from("packages/app/qlang.toml"),
            package_name: "app".to_owned(),
            default_profile: None,
            targets,
        }
    }

    #[test]
    fn display_path_selector_matches_package_relative_targets() {
        let selector = ProjectTargetSelector {
            package_name: Some("app".to_owned()),
            target: Some(ProjectTargetSelectorKind::DisplayPath(
                "src/main.ql".to_owned(),
            )),
        };
        let target = BuildTarget {
            kind: BuildTargetKind::Binary,
            path: PathBuf::from("packages/app/src/main.ql"),
        };

        assert!(selector.matches(Path::new("packages/app/qlang.toml"), "app", &target));
        assert!(!selector.matches(Path::new("packages/app/qlang.toml"), "other", &target));
    }

    #[test]
    fn filter_can_preserve_empty_members_for_list_views() {
        let members = vec![member_with_targets(vec![BuildTarget {
            kind: BuildTargetKind::Library,
            path: PathBuf::from("packages/app/src/lib.ql"),
        }])];

        let dropped = filter_workspace_build_targets(&members, false, |target| {
            target.kind == BuildTargetKind::Binary
        });
        let preserved = filter_workspace_build_targets(&members, true, |target| {
            target.kind == BuildTargetKind::Binary
        });

        assert!(dropped.is_empty());
        assert_eq!(preserved.len(), 1);
        assert!(preserved[0].targets.is_empty());
    }

    #[test]
    fn targets_json_renders_stable_schema_and_display_paths() {
        let members = vec![member_with_targets(vec![BuildTarget {
            kind: BuildTargetKind::Binary,
            path: PathBuf::from("packages/app/src/main.ql"),
        }])];

        let rendered = render_project_targets_json(&members);

        assert!(rendered.contains("\"schema\": \"ql.project.targets.v1\""));
        assert!(rendered.contains("\"package_name\": \"app\""));
        assert!(rendered.contains("\"path\": \"src/main.ql\""));
    }
}
