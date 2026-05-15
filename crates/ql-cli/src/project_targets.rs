use std::path::{Path, PathBuf};

use ql_project::{
    BuildTarget, BuildTargetKind, WorkspaceBuildTargets, discover_package_build_targets,
    load_project_manifest, package_name,
};

use super::{
    is_ql_source_file, json_string, load_workspace_build_targets_for_command_from_request_root,
    normalize_path,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProjectTargetSelector {
    pub(crate) package_name: Option<String>,
    pub(crate) target: Option<ProjectTargetSelectorKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectTargetSelectorKind {
    Library,
    Binary(String),
    DisplayPath(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCommandScope {
    Project,
    ProjectBuildTarget(ProjectSourceBuildTargetRequest),
    ProjectTestFile(ProjectFileTestRequest),
    DirectSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCheckCommandScope {
    Project {
        request_root_manifest_path: Option<PathBuf>,
    },
    DirectSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedProjectCommandPath {
    Project {
        request_root_manifest_path: Option<PathBuf>,
        selector: ProjectTargetSelector,
    },
    DirectSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCommandPathError {
    SourcePathRejectsSelector,
    SelectorRequiresProjectContext,
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

pub(crate) fn resolve_project_command_path(
    path: &Path,
    selector: &ProjectTargetSelector,
) -> Result<ResolvedProjectCommandPath, ProjectCommandPathError> {
    match resolve_project_command_scope(path) {
        ProjectCommandScope::Project => Ok(ResolvedProjectCommandPath::Project {
            request_root_manifest_path: None,
            selector: selector.clone(),
        }),
        ProjectCommandScope::ProjectBuildTarget(request) => {
            if selector.is_active() {
                return Err(ProjectCommandPathError::SourcePathRejectsSelector);
            }
            Ok(ResolvedProjectCommandPath::Project {
                request_root_manifest_path: Some(request.request_root_manifest_path),
                selector: request.selector,
            })
        }
        ProjectCommandScope::ProjectTestFile(_) | ProjectCommandScope::DirectSource => {
            if selector.is_active() {
                Err(ProjectCommandPathError::SelectorRequiresProjectContext)
            } else {
                Ok(ResolvedProjectCommandPath::DirectSource)
            }
        }
    }
}

pub(crate) fn resolve_project_command_scope(path: &Path) -> ProjectCommandScope {
    if is_project_context_path(path) {
        return ProjectCommandScope::Project;
    }
    if let Some(request) = resolve_project_source_build_target_request(path) {
        return ProjectCommandScope::ProjectBuildTarget(request);
    }
    if let Some(request) = resolve_project_file_test_request(path) {
        return ProjectCommandScope::ProjectTestFile(request);
    }
    ProjectCommandScope::DirectSource
}

pub(crate) fn resolve_project_check_command_scope(path: &Path) -> ProjectCheckCommandScope {
    if is_project_context_path(path) {
        return ProjectCheckCommandScope::Project {
            request_root_manifest_path: resolve_project_workspace_member_command_request_root(path),
        };
    }

    if let Some(request_root_manifest_path) =
        resolve_project_workspace_member_command_request_root(path)
    {
        return ProjectCheckCommandScope::Project {
            request_root_manifest_path: Some(request_root_manifest_path),
        };
    }

    ProjectCheckCommandScope::DirectSource
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectSourceBuildTargetRequest {
    pub(crate) request_root_manifest_path: PathBuf,
    pub(crate) selector: ProjectTargetSelector,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectFileTestRequest {
    pub(crate) request_root_manifest_path: PathBuf,
    pub(crate) display_path: String,
}

pub(crate) fn project_request_root(path: &Path) -> PathBuf {
    if is_project_manifest_path(path) {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        path.to_path_buf()
    }
}

pub(crate) fn resolve_project_member_request_root(package_manifest_path: &Path) -> PathBuf {
    find_enclosing_workspace_manifest_for_member(package_manifest_path)
        .unwrap_or_else(|| package_manifest_path.to_path_buf())
}

pub(crate) fn display_relative_to_root(root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(root) {
        normalize_path(relative)
    } else {
        normalize_path(path)
    }
}

pub(crate) fn resolve_project_workspace_member_command_request_root(
    path: &Path,
) -> Option<PathBuf> {
    if !path.is_dir() && !is_ql_source_file(path) && !is_project_manifest_path(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    Some(resolve_project_member_request_root(&manifest.manifest_path))
}

pub(crate) fn resolve_project_source_build_target_request(
    path: &Path,
) -> Option<ProjectSourceBuildTargetRequest> {
    if !is_ql_source_file(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    let package_name = package_name(&manifest).ok()?.to_owned();
    let display_path = project_target_display_path(&manifest.manifest_path, path);
    let targets = discover_package_build_targets(&manifest).ok()?;
    if !targets.iter().any(|target| {
        project_target_display_path(&manifest.manifest_path, target.path.as_path()) == display_path
    }) {
        return None;
    }

    let request_root_manifest_path = resolve_project_member_request_root(&manifest.manifest_path);

    Some(ProjectSourceBuildTargetRequest {
        request_root_manifest_path,
        selector: ProjectTargetSelector {
            package_name: Some(package_name),
            target: Some(ProjectTargetSelectorKind::DisplayPath(display_path)),
        },
    })
}

pub(crate) fn resolve_project_file_test_request(path: &Path) -> Option<ProjectFileTestRequest> {
    if !is_ql_source_file(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    let _ = package_name(&manifest).ok()?;
    let manifest_path = manifest.manifest_path.clone();
    let package_root = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let tests_root = package_root.join("tests");
    path.strip_prefix(&tests_root).ok()?;
    let request_root_manifest_path = resolve_project_member_request_root(&manifest_path);
    let request_root = project_request_root(&request_root_manifest_path);

    Some(ProjectFileTestRequest {
        request_root_manifest_path,
        display_path: display_relative_to_root(&request_root, path),
    })
}

fn is_project_context_path(path: &Path) -> bool {
    path.is_dir() || is_project_manifest_path(path)
}

fn is_project_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
}

fn find_enclosing_workspace_manifest_for_member(package_manifest_path: &Path) -> Option<PathBuf> {
    let package_root = package_manifest_path.parent()?;
    let mut current = package_root.parent().map(Path::to_path_buf);

    while let Some(directory) = current {
        let candidate = directory.join("qlang.toml");
        if candidate.is_file()
            && let Ok(manifest) = load_project_manifest(&candidate)
            && workspace_manifest_contains_member(&manifest, package_manifest_path)
        {
            return Some(manifest.manifest_path);
        }
        current = directory.parent().map(Path::to_path_buf);
    }

    None
}

fn workspace_manifest_contains_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest_path: &Path,
) -> bool {
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return false;
    };

    let expected_manifest_path = normalize_path(package_manifest_path);
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    workspace.members.iter().any(|member| {
        load_project_manifest(&workspace_root.join(member))
            .ok()
            .is_some_and(|member_manifest| {
                normalize_path(&member_manifest.manifest_path) == expected_manifest_path
            })
    })
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
    select_workspace_build_targets_with_failure(path, members, selector, target_label).map_err(
        |failure| {
            report_project_target_selection_failure(command_label, &failure);
            1
        },
    )
}

fn select_workspace_build_targets_with_failure(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, ProjectTargetSelectionFailure> {
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
        return Err(ProjectTargetSelectionFailure {
            stage: "target-selection",
            path: normalize_path(path),
            message: format!(
                "target selector matched no {target_label} under `{}`",
                normalize_path(path)
            ),
            selector: selector.describe(),
            target_count: members
                .iter()
                .map(|member| member.targets.len())
                .sum::<usize>(),
        });
    }

    Ok(selected)
}

struct ProjectTargetSelectionFailure {
    stage: &'static str,
    path: String,
    message: String,
    selector: String,
    target_count: usize,
}

fn report_project_target_selection_failure(
    command_label: &str,
    failure: &ProjectTargetSelectionFailure,
) {
    eprintln!("error: {command_label} {}", failure.message);
    eprintln!("note: selector: {}", failure.selector);
    eprintln!(
        "hint: rerun `ql project targets {}` to inspect the discovered build targets",
        failure.path
    );
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
    let members = match select_workspace_build_targets_with_failure(
        path,
        &members,
        selector,
        "build targets",
    ) {
        Ok(members) => members,
        Err(failure) => {
            if json {
                print!(
                    "{}",
                    render_project_targets_selection_failure_json(&failure)
                );
            } else {
                report_project_target_selection_failure("`ql project targets`", &failure);
            }
            return Err(1);
        }
    };
    render_project_target_members(&members, json);
    Ok(())
}

pub(crate) fn list_build_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members = load_project_target_members_for_workspace_member_path(path, "`ql build --list`")?;
    let members = match select_workspace_build_targets_with_failure(
        path,
        &members,
        selector,
        "build targets",
    ) {
        Ok(members) => members,
        Err(failure) => {
            if json {
                print!(
                    "{}",
                    render_project_targets_selection_failure_json(&failure)
                );
            } else {
                report_project_target_selection_failure("`ql build --list`", &failure);
            }
            return Err(1);
        }
    };
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
        match select_workspace_build_targets_with_failure(path, &members, selector, "build targets")
        {
            Ok(members) => members,
            Err(failure) => {
                if json {
                    print!(
                        "{}",
                        render_project_targets_selection_failure_json(&failure)
                    );
                } else {
                    report_project_target_selection_failure("`ql run --list`", &failure);
                }
                return Err(1);
            }
        }
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
        let failure = ProjectTargetSelectionFailure {
            stage: "runnable-selection",
            path: normalize_path(path),
            message: format!(
                "target selector matched no runnable build targets under `{}`",
                normalize_path(path)
            ),
            selector: selector.describe(),
            target_count: selected
                .iter()
                .map(|member| member.targets.len())
                .sum::<usize>(),
        };
        if json {
            print!(
                "{}",
                render_project_targets_selection_failure_json(&failure)
            );
        } else {
            report_project_target_selection_failure("`ql run --list`", &failure);
        }
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

fn render_project_targets_selection_failure_json(
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

    #[test]
    fn command_path_resolution_requires_project_context_for_selectors() {
        let selector = ProjectTargetSelector {
            package_name: Some("app".to_owned()),
            target: Some(ProjectTargetSelectorKind::Library),
        };

        let resolved = resolve_project_command_path(Path::new("sample.ql"), &selector);

        assert_eq!(
            resolved,
            Err(ProjectCommandPathError::SelectorRequiresProjectContext)
        );
    }

    #[test]
    fn command_path_resolution_preserves_manifest_selector_context() {
        let selector = ProjectTargetSelector {
            package_name: Some("app".to_owned()),
            target: Some(ProjectTargetSelectorKind::Binary("admin".to_owned())),
        };

        let resolved =
            resolve_project_command_path(Path::new("packages/app/qlang.toml"), &selector);

        assert_eq!(
            resolved,
            Ok(ResolvedProjectCommandPath::Project {
                request_root_manifest_path: None,
                selector,
            })
        );
    }

    #[test]
    fn command_scope_resolution_treats_manifest_paths_as_project_context() {
        let resolved = resolve_project_command_scope(Path::new("packages/app/qlang.toml"));

        assert_eq!(resolved, ProjectCommandScope::Project);
    }
}
