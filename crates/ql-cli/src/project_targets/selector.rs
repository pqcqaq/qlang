use std::path::Path;

use ql_project::{BuildTarget, BuildTargetKind};

use crate::cli_utils::normalize_path;

use super::project_target_display_path;

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
