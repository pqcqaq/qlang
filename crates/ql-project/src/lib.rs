use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use toml::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageManifest {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceManifest {
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferencesManifest {
    pub packages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectManifest {
    pub manifest_path: PathBuf,
    pub package: Option<PackageManifest>,
    pub workspace: Option<WorkspaceManifest>,
    pub references: ReferencesManifest,
}

#[derive(Debug)]
pub enum ProjectError {
    ManifestNotFound {
        start: PathBuf,
    },
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestNotFound { start } => write!(
                f,
                "could not find `qlang.toml` starting from `{}`",
                display_path(start)
            ),
            Self::Read { path, error } => write!(
                f,
                "failed to read manifest `{}`: {error}",
                display_path(path)
            ),
            Self::Parse { path, message } => {
                write!(f, "invalid manifest `{}`: {message}", display_path(path))
            }
        }
    }
}

pub fn load_project_manifest(path: &Path) -> Result<ProjectManifest, ProjectError> {
    let manifest_path = find_manifest_path(path)?;
    let source = fs::read_to_string(&manifest_path).map_err(|error| ProjectError::Read {
        path: manifest_path.clone(),
        error,
    })?;
    let value = toml::from_str::<Value>(&source).map_err(|error| ProjectError::Parse {
        path: manifest_path.clone(),
        message: error.to_string(),
    })?;
    let Some(root) = value.as_table() else {
        return Err(ProjectError::Parse {
            path: manifest_path.clone(),
            message: "top-level manifest must be a TOML table".to_owned(),
        });
    };

    let package = parse_package(root, &manifest_path)?;
    let workspace = parse_workspace(root, &manifest_path)?;
    let references = parse_references(root, &manifest_path)?;

    if package.is_none() && workspace.is_none() {
        return Err(ProjectError::Parse {
            path: manifest_path.clone(),
            message: "`qlang.toml` requires `[package]` or `[workspace]`".to_owned(),
        });
    }

    Ok(ProjectManifest {
        manifest_path,
        package,
        workspace,
        references,
    })
}

pub fn render_project_graph(manifest: &ProjectManifest) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "manifest: {}\n",
        display_path(&manifest.manifest_path)
    ));
    output.push_str(&format!(
        "package: {}\n",
        manifest
            .package
            .as_ref()
            .map(|package| package.name.as_str())
            .unwrap_or("<none>")
    ));

    if let Some(workspace) = &manifest.workspace {
        output.push_str("workspace_members:\n");
        for member in &workspace.members {
            output.push_str(&format!("  - {member}\n"));
        }
    } else {
        output.push_str("workspace_members: []\n");
    }

    if manifest.references.packages.is_empty() {
        output.push_str("references: []\n");
    } else {
        output.push_str("references:\n");
        for package in &manifest.references.packages {
            output.push_str(&format!("  - {package}\n"));
        }
    }

    output
}

fn find_manifest_path(path: &Path) -> Result<PathBuf, ProjectError> {
    let start = if path.as_os_str().is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        path.to_path_buf()
    };

    if start.is_file()
        && start
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return Ok(start);
    }

    let mut current = if start.is_dir() {
        start.clone()
    } else {
        start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };

    loop {
        let candidate = current.join("qlang.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }

    Err(ProjectError::ManifestNotFound { start })
}

fn parse_package(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Option<PackageManifest>, ProjectError> {
    let Some(package) = root.get("package") else {
        return Ok(None);
    };
    let Some(package) = package.as_table() else {
        return Err(parse_error(
            manifest_path,
            "`[package]` must be a TOML table",
        ));
    };
    let Some(name) = package.get("name") else {
        return Err(parse_error(
            manifest_path,
            "`[package].name` must be present",
        ));
    };
    let Some(name) = name.as_str() else {
        return Err(parse_error(
            manifest_path,
            "`[package].name` must be a string",
        ));
    };

    Ok(Some(PackageManifest {
        name: name.to_owned(),
    }))
}

fn parse_workspace(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Option<WorkspaceManifest>, ProjectError> {
    let Some(workspace) = root.get("workspace") else {
        return Ok(None);
    };
    let Some(workspace) = workspace.as_table() else {
        return Err(parse_error(
            manifest_path,
            "`[workspace]` must be a TOML table",
        ));
    };
    let members = parse_string_array(
        workspace.get("members"),
        "`[workspace].members`",
        manifest_path,
    )?
    .unwrap_or_default();
    Ok(Some(WorkspaceManifest { members }))
}

fn parse_references(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<ReferencesManifest, ProjectError> {
    let Some(references) = root.get("references") else {
        return Ok(ReferencesManifest::default());
    };
    let Some(references) = references.as_table() else {
        return Err(parse_error(
            manifest_path,
            "`[references]` must be a TOML table",
        ));
    };
    let packages = parse_string_array(
        references.get("packages"),
        "`[references].packages`",
        manifest_path,
    )?
    .unwrap_or_default();
    Ok(ReferencesManifest { packages })
}

fn parse_string_array(
    value: Option<&Value>,
    field_name: &str,
    manifest_path: &Path,
) -> Result<Option<Vec<String>>, ProjectError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(items) = value.as_array() else {
        return Err(parse_error(
            manifest_path,
            format!("{field_name} must be an array of strings"),
        ));
    };

    let mut values = Vec::with_capacity(items.len());
    for item in items {
        let Some(item) = item.as_str() else {
            return Err(parse_error(
                manifest_path,
                format!("{field_name} must be an array of strings"),
            ));
        };
        values.push(item.to_owned());
    }
    Ok(Some(values))
}

fn parse_error(manifest_path: &Path, message: impl Into<String>) -> ProjectError {
    ProjectError::Parse {
        path: manifest_path.to_path_buf(),
        message: message.into(),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
