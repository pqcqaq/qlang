use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use ql_ast::{
    EnumDecl, ExtendBlock, ExternBlock, FunctionDecl, GenericParam, GlobalDecl, ImplBlock, Item,
    ItemKind, Module, Param, Path as AstPath, ReceiverKind, StructDecl, TraitDecl, TypeAliasDecl,
    TypeExpr, TypeExprKind, UseDecl, VariantFields, Visibility, WherePredicate,
};
use ql_lexer::is_keyword as lexer_is_keyword;
use ql_parser::parse_interface_source;
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

#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceModule {
    pub source_path: String,
    pub contents: String,
    pub syntax: Module,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceArtifact {
    pub package_name: String,
    pub modules: Vec<InterfaceModule>,
}

#[derive(Debug)]
pub enum ProjectError {
    ManifestNotFound {
        start: PathBuf,
    },
    PackageNotDefined {
        path: PathBuf,
    },
    PackageSourceRootNotFound {
        path: PathBuf,
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

#[derive(Debug)]
pub enum InterfaceError {
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
            Self::PackageNotDefined { path } => write!(
                f,
                "manifest `{}` does not declare `[package].name`",
                display_path(path)
            ),
            Self::PackageSourceRootNotFound { path } => write!(
                f,
                "package source directory `{}` does not exist",
                display_path(path)
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

impl fmt::Display for InterfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, error } => write!(
                f,
                "failed to read interface `{}`: {error}",
                display_path(path)
            ),
            Self::Parse { path, message } => {
                write!(f, "invalid interface `{}`: {message}", display_path(path))
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
    let manifest_dir = manifest_dir(manifest);
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

    if manifest.package.is_some() {
        let interface_path =
            default_interface_path(manifest).expect("package manifests have a default interface");
        append_interface_summary(&mut output, manifest_dir, manifest, &interface_path, "");
        append_reference_interface_summaries(&mut output, manifest_dir, manifest, "");
    }

    output
}

pub fn render_project_graph_resolved(manifest: &ProjectManifest) -> Result<String, ProjectError> {
    let mut output = render_project_graph(manifest);

    if manifest.package.is_some() || manifest.workspace.is_none() {
        return Ok(output);
    }

    let manifest_dir = manifest_dir(manifest);
    output.push_str("workspace_packages:\n");
    for member in &manifest
        .workspace
        .as_ref()
        .expect("workspace exists")
        .members
    {
        let member_path = manifest_dir.join(member);
        let member_manifest = match load_project_manifest(&member_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                append_workspace_member_error(
                    &mut output,
                    manifest_dir,
                    member,
                    &project_manifest_path(&member_path),
                    &error,
                );
                continue;
            }
        };
        let package = match package_name(&member_manifest) {
            Ok(package) => package,
            Err(error) => {
                append_workspace_member_error(
                    &mut output,
                    manifest_dir,
                    member,
                    &member_manifest.manifest_path,
                    &error,
                );
                continue;
            }
        };
        let interface_path = match default_interface_path(&member_manifest) {
            Ok(path) => path,
            Err(error) => {
                append_workspace_member_error(
                    &mut output,
                    manifest_dir,
                    member,
                    &member_manifest.manifest_path,
                    &error,
                );
                continue;
            }
        };

        output.push_str(&format!("  - member: {member}\n"));
        output.push_str(&format!(
            "    manifest: {}\n",
            relative_display_path(manifest_dir, &member_manifest.manifest_path)
        ));
        output.push_str(&format!("    package: {package}\n"));
        append_interface_summary(
            &mut output,
            manifest_dir,
            &member_manifest,
            &interface_path,
            "    ",
        );

        if member_manifest.references.packages.is_empty() {
            output.push_str("    references: []\n");
        } else {
            output.push_str("    references:\n");
            for reference in &member_manifest.references.packages {
                output.push_str(&format!("      - {reference}\n"));
            }
        }

        append_reference_interface_summaries(&mut output, manifest_dir, &member_manifest, "    ");
    }

    Ok(output)
}

fn append_workspace_member_error(
    output: &mut String,
    root: &Path,
    member: &str,
    manifest_path: &Path,
    error: &ProjectError,
) {
    output.push_str(&format!("  - member: {member}\n"));
    output.push_str(&format!(
        "    manifest: {}\n",
        relative_display_path(root, manifest_path)
    ));
    output.push_str("    package: <unresolved>\n");
    output.push_str(&format!("    member_error: {error}\n"));
}

fn project_manifest_path(path: &Path) -> PathBuf {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
    {
        return path.to_path_buf();
    }

    path.join("qlang.toml")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceArtifactStatus {
    Missing,
    Unreadable,
    Invalid,
    Stale,
    Valid,
}

impl InterfaceArtifactStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
            Self::Invalid => "invalid",
            Self::Stale => "stale",
            Self::Valid => "valid",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceArtifactStaleReason {
    ManifestNewer { path: PathBuf },
    SourceNewer { path: PathBuf },
}

impl InterfaceArtifactStaleReason {
    fn label(&self) -> &'static str {
        match self {
            Self::ManifestNewer { .. } => "manifest",
            Self::SourceNewer { .. } => "source",
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::ManifestNewer { path } | Self::SourceNewer { path } => path,
        }
    }
}

pub fn interface_artifact_status_detail(
    path: &Path,
    status: InterfaceArtifactStatus,
) -> Option<String> {
    match load_interface_artifact(path) {
        Err(InterfaceError::Read { error, .. })
            if status == InterfaceArtifactStatus::Unreadable =>
        {
            Some(error.to_string())
        }
        Err(InterfaceError::Parse { message, .. })
            if status == InterfaceArtifactStatus::Invalid =>
        {
            Some(message)
        }
        _ => None,
    }
}

fn append_interface_summary(
    output: &mut String,
    root: &Path,
    manifest: &ProjectManifest,
    interface_path: &Path,
    indent: &str,
) {
    let status = interface_artifact_status(manifest, interface_path);
    output.push_str(&format!("{indent}interface:\n"));
    output.push_str(&format!(
        "{indent}  path: {}\n",
        relative_display_path(root, interface_path)
    ));
    output.push_str(&format!("{indent}  status: {}\n", status.label()));
    if let Some(detail) = interface_artifact_status_detail(interface_path, status) {
        output.push_str(&format!("{indent}  detail: {detail}\n"));
    }
    if status == InterfaceArtifactStatus::Stale {
        let stale_reasons = interface_artifact_stale_reasons(manifest, interface_path);
        let stale_indent = format!("{indent}  ");
        append_stale_reason_summary(output, root, &stale_reasons, &stale_indent);
    }
}

fn append_reference_interface_summaries(
    output: &mut String,
    root: &Path,
    manifest: &ProjectManifest,
    indent: &str,
) {
    if manifest.references.packages.is_empty() {
        output.push_str(&format!("{indent}reference_interfaces: []\n"));
        return;
    }

    let manifest_dir = manifest_dir(manifest);
    output.push_str(&format!("{indent}reference_interfaces:\n"));
    for reference in &manifest.references.packages {
        let reference_manifest_path = project_manifest_path(&manifest_dir.join(reference));
        output.push_str(&format!("{indent}  - reference: {reference}\n"));
        output.push_str(&format!(
            "{indent}    manifest: {}\n",
            relative_display_path(root, &reference_manifest_path)
        ));
        match load_project_manifest(&manifest_dir.join(reference)) {
            Ok(reference_manifest) => match default_interface_path(&reference_manifest) {
                Ok(interface_path) => {
                    let status = interface_artifact_status(&reference_manifest, &interface_path);
                    let package = reference_manifest
                        .package
                        .as_ref()
                        .map(|package| package.name.as_str())
                        .unwrap_or("<unresolved>");
                    output.push_str(&format!("{indent}    package: {package}\n"));
                    output.push_str(&format!(
                        "{indent}    path: {}\n",
                        relative_display_path(root, &interface_path)
                    ));
                    output.push_str(&format!("{indent}    status: {}\n", status.label()));
                    let transitive_reference_failures =
                        count_transitive_reference_failures(&reference_manifest);
                    if transitive_reference_failures > 0 {
                        output.push_str(&format!(
                            "{indent}    transitive_reference_failures: {transitive_reference_failures}\n"
                        ));
                    }
                    if let Some(detail) = interface_artifact_status_detail(&interface_path, status)
                    {
                        output.push_str(&format!("{indent}    detail: {detail}\n"));
                    }
                    if status == InterfaceArtifactStatus::Stale {
                        let stale_reasons =
                            interface_artifact_stale_reasons(&reference_manifest, &interface_path);
                        let stale_indent = format!("{indent}    ");
                        append_stale_reason_summary(output, root, &stale_reasons, &stale_indent);
                    }
                }
                Err(error) => {
                    output.push_str(&format!("{indent}    package: <unresolved>\n"));
                    output.push_str(&format!("{indent}    path: <unresolved>\n"));
                    output.push_str(&format!("{indent}    status: unresolved-package\n"));
                    output.push_str(&format!("{indent}    detail: {error}\n"));
                }
            },
            Err(error) => {
                output.push_str(&format!("{indent}    package: <unresolved>\n"));
                output.push_str(&format!("{indent}    path: <unresolved>\n"));
                output.push_str(&format!("{indent}    status: unresolved-manifest\n"));
                output.push_str(&format!("{indent}    detail: {error}\n"));
            }
        }
    }
}

fn count_transitive_reference_failures(manifest: &ProjectManifest) -> usize {
    let mut visited = BTreeSet::new();
    count_reference_failures_recursive(manifest, &mut visited)
}

fn count_reference_failures_recursive(
    manifest: &ProjectManifest,
    visited: &mut BTreeSet<PathBuf>,
) -> usize {
    let manifest_path = manifest.manifest_path.clone();
    if !visited.insert(manifest_path) {
        return 0;
    }

    let manifest_dir = manifest_dir(manifest);
    let mut failure_count = 0usize;
    for reference in &manifest.references.packages {
        match load_project_manifest(&manifest_dir.join(reference)) {
            Ok(reference_manifest) => {
                let interface_path = default_interface_path(&reference_manifest);
                match interface_path {
                    Ok(interface_path) => {
                        if interface_artifact_status(&reference_manifest, &interface_path)
                            != InterfaceArtifactStatus::Valid
                        {
                            failure_count += 1;
                        }
                    }
                    Err(_) => failure_count += 1,
                }
                failure_count += count_reference_failures_recursive(&reference_manifest, visited);
            }
            Err(_) => failure_count += 1,
        }
    }

    failure_count
}

fn append_stale_reason_summary(
    output: &mut String,
    root: &Path,
    stale_reasons: &[InterfaceArtifactStaleReason],
    indent: &str,
) {
    if stale_reasons.is_empty() {
        return;
    }

    output.push_str(&format!("{indent}stale_reasons:\n"));
    for reason in stale_reasons {
        output.push_str(&format!(
            "{indent}  - {}: {}\n",
            reason.label(),
            relative_display_path(root, reason.path())
        ));
    }
}

pub fn interface_artifact_status(
    manifest: &ProjectManifest,
    path: &Path,
) -> InterfaceArtifactStatus {
    if !path.is_file() {
        return InterfaceArtifactStatus::Missing;
    }

    match load_interface_artifact(path) {
        Ok(_) => {
            if interface_artifact_is_stale(manifest, path) {
                InterfaceArtifactStatus::Stale
            } else {
                InterfaceArtifactStatus::Valid
            }
        }
        Err(InterfaceError::Parse { .. }) => InterfaceArtifactStatus::Invalid,
        Err(InterfaceError::Read { .. }) => InterfaceArtifactStatus::Unreadable,
    }
}

pub fn interface_artifact_stale_reasons(
    manifest: &ProjectManifest,
    interface_path: &Path,
) -> Vec<InterfaceArtifactStaleReason> {
    let interface_modified = match file_modified(interface_path) {
        Some(modified) => modified,
        None => return Vec::new(),
    };

    let mut stale_reasons = Vec::new();
    if file_modified(&manifest.manifest_path).is_some_and(|modified| modified > interface_modified)
    {
        stale_reasons.push(InterfaceArtifactStaleReason::ManifestNewer {
            path: manifest.manifest_path.clone(),
        });
    }

    if let Ok(source_paths) = collect_package_sources(manifest) {
        if let Some(path) = source_paths
            .into_iter()
            .find(|path| file_modified(path).is_some_and(|modified| modified > interface_modified))
        {
            stale_reasons.push(InterfaceArtifactStaleReason::SourceNewer { path });
        }
    }

    stale_reasons
}

fn interface_artifact_is_stale(manifest: &ProjectManifest, interface_path: &Path) -> bool {
    !interface_artifact_stale_reasons(manifest, interface_path).is_empty()
}

fn file_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

pub fn manifest_dir(manifest: &ProjectManifest) -> &Path {
    manifest.manifest_path.parent().unwrap_or(Path::new("."))
}

pub fn package_name(manifest: &ProjectManifest) -> Result<&str, ProjectError> {
    manifest
        .package
        .as_ref()
        .map(|package| package.name.as_str())
        .ok_or_else(|| ProjectError::PackageNotDefined {
            path: manifest.manifest_path.clone(),
        })
}

pub fn package_source_root(manifest: &ProjectManifest) -> Result<PathBuf, ProjectError> {
    let _ = package_name(manifest)?;
    Ok(manifest_dir(manifest).join("src"))
}

pub fn collect_package_sources(manifest: &ProjectManifest) -> Result<Vec<PathBuf>, ProjectError> {
    let source_root = package_source_root(manifest)?;
    if !source_root.is_dir() {
        return Err(ProjectError::PackageSourceRootNotFound { path: source_root });
    }

    let mut files = Vec::new();
    collect_package_sources_recursive(&source_root, &mut files)?;
    files.sort();
    Ok(files)
}

pub fn default_interface_path(manifest: &ProjectManifest) -> Result<PathBuf, ProjectError> {
    let package_name = package_name(manifest)?;
    Ok(manifest_dir(manifest).join(format!("{package_name}.qi")))
}

pub fn load_interface_artifact(path: &Path) -> Result<InterfaceArtifact, InterfaceError> {
    let source = fs::read_to_string(path).map_err(|error| InterfaceError::Read {
        path: path.to_path_buf(),
        error,
    })?;
    parse_interface_artifact(path, &source)
}

pub fn load_reference_manifests(
    manifest: &ProjectManifest,
) -> Result<Vec<ProjectManifest>, ProjectError> {
    let manifest_dir = manifest_dir(manifest);
    let mut references = Vec::with_capacity(manifest.references.packages.len());
    for package in &manifest.references.packages {
        references.push(load_project_manifest(&manifest_dir.join(package))?);
    }
    Ok(references)
}

pub fn render_module_interface(module: &Module) -> Option<String> {
    let rendered_items = module
        .items
        .iter()
        .filter_map(render_item_interface)
        .collect::<Vec<_>>();
    if rendered_items.is_empty() {
        return None;
    }

    let mut out = String::new();

    if let Some(package) = &module.package {
        out.push_str("package ");
        format_path(&package.path, &mut out);
        out.push('\n');
    }

    if !module.uses.is_empty() {
        if module.package.is_some() {
            out.push('\n');
        }
        for use_decl in &module.uses {
            render_use_decl(use_decl, &mut out);
            out.push('\n');
        }
    }

    if module.package.is_some() || !module.uses.is_empty() {
        out.push('\n');
    }

    for (index, item) in rendered_items.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n");
        }
        out.push_str(item);
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }

    Some(out)
}

fn collect_package_sources_recursive(
    path: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ProjectError> {
    for entry in fs::read_dir(path).map_err(|error| ProjectError::Read {
        path: path.to_path_buf(),
        error,
    })? {
        let entry = entry.map_err(|error| ProjectError::Read {
            path: path.to_path_buf(),
            error,
        })?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_package_sources_recursive(&entry_path, files)?;
        } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("ql") {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn parse_interface_artifact(
    path: &Path,
    source: &str,
) -> Result<InterfaceArtifact, InterfaceError> {
    let normalized = source.replace("\r\n", "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut index = 0;

    let version_line =
        next_nonempty_line(&lines, &mut index).ok_or_else(|| InterfaceError::Parse {
            path: path.to_path_buf(),
            message: "missing `// qlang interface v1` header".to_owned(),
        })?;
    if version_line != "// qlang interface v1" {
        return Err(InterfaceError::Parse {
            path: path.to_path_buf(),
            message: "expected `// qlang interface v1` header".to_owned(),
        });
    }

    let package_line =
        next_nonempty_line(&lines, &mut index).ok_or_else(|| InterfaceError::Parse {
            path: path.to_path_buf(),
            message: "missing `// package: ...` header".to_owned(),
        })?;
    let Some(package_name) = package_line.strip_prefix("// package: ") else {
        return Err(InterfaceError::Parse {
            path: path.to_path_buf(),
            message: "expected `// package: ...` header".to_owned(),
        });
    };

    let mut modules = Vec::new();
    let mut current_source = None::<String>;
    let mut current_lines = Vec::new();

    while index < lines.len() {
        let line = lines[index];
        index += 1;
        if let Some(source_path) = line.strip_prefix("// source: ") {
            if let Some(source_path) = current_source.take() {
                let contents = finalize_interface_module(&current_lines);
                let syntax = parse_interface_module(path, &source_path, &contents)?;
                modules.push(InterfaceModule {
                    source_path,
                    syntax,
                    contents,
                });
                current_lines.clear();
            }
            current_source = Some(source_path.to_owned());
            continue;
        }

        if current_source.is_none() {
            if line.trim().is_empty() {
                continue;
            }
            return Err(InterfaceError::Parse {
                path: path.to_path_buf(),
                message: "unexpected content before first `// source: ...` section".to_owned(),
            });
        }

        current_lines.push(line);
    }

    if let Some(source_path) = current_source {
        let contents = finalize_interface_module(&current_lines);
        let syntax = parse_interface_module(path, &source_path, &contents)?;
        modules.push(InterfaceModule {
            source_path,
            syntax,
            contents,
        });
    }

    Ok(InterfaceArtifact {
        package_name: package_name.to_owned(),
        modules,
    })
}

fn next_nonempty_line<'a>(lines: &'a [&str], index: &mut usize) -> Option<&'a str> {
    while *index < lines.len() {
        let line = lines[*index];
        *index += 1;
        if !line.trim().is_empty() {
            return Some(line);
        }
    }
    None
}

fn finalize_interface_module(lines: &[&str]) -> String {
    let joined = lines.join("\n");
    joined.trim_start_matches('\n').trim().to_owned()
}

fn parse_interface_module(
    path: &Path,
    source_path: &str,
    contents: &str,
) -> Result<Module, InterfaceError> {
    parse_interface_source(contents).map_err(|errors| InterfaceError::Parse {
        path: path.to_path_buf(),
        message: format!(
            "failed to parse interface section `{source_path}`: {}",
            errors
                .into_iter()
                .map(|error| format!("{} @ {}", error.message, error.span))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    })
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

fn render_item_interface(item: &Item) -> Option<String> {
    let mut out = String::new();
    let rendered = match &item.kind {
        ItemKind::Function(function) if is_public(&function.visibility) => {
            render_function_signature(function, 0, true, &mut out);
            true
        }
        ItemKind::Const(global) if is_public(&global.visibility) => {
            render_global_signature("const", global, 0, &mut out);
            true
        }
        ItemKind::Static(global) if is_public(&global.visibility) => {
            render_global_signature("static", global, 0, &mut out);
            true
        }
        ItemKind::Struct(struct_decl) if is_public(&struct_decl.visibility) => {
            render_struct_signature(struct_decl, 0, &mut out);
            true
        }
        ItemKind::Enum(enum_decl) if is_public(&enum_decl.visibility) => {
            render_enum_signature(enum_decl, 0, &mut out);
            true
        }
        ItemKind::Trait(trait_decl) if is_public(&trait_decl.visibility) => {
            render_trait_signature(trait_decl, 0, &mut out);
            true
        }
        ItemKind::Impl(impl_block) => render_impl_signature(impl_block, 0, &mut out),
        ItemKind::Extend(extend_block) => render_extend_signature(extend_block, 0, &mut out),
        ItemKind::TypeAlias(type_alias) if is_public(&type_alias.visibility) => {
            render_type_alias_signature(type_alias, 0, &mut out);
            true
        }
        ItemKind::ExternBlock(extern_block) if is_public(&extern_block.visibility) => {
            render_extern_block_signature(extern_block, 0, &mut out);
            true
        }
        _ => false,
    };

    rendered.then_some(out)
}

fn render_use_decl(use_decl: &UseDecl, out: &mut String) {
    out.push_str("use ");
    format_path(&use_decl.prefix, out);
    if let Some(group) = &use_decl.group {
        out.push_str(".{");
        for (index, item) in group.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            format_ident(&item.name, out);
            if let Some(alias) = &item.alias {
                out.push_str(" as ");
                format_ident(alias, out);
            }
        }
        out.push('}');
    }
    if let Some(alias) = &use_decl.alias {
        out.push_str(" as ");
        format_ident(alias, out);
    }
}

fn render_global_signature(keyword: &str, global: &GlobalDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&global.visibility, out);
    out.push_str(keyword);
    out.push(' ');
    format_ident(&global.name, out);
    out.push_str(": ");
    format_type(&global.ty, out);
}

fn render_struct_signature(struct_decl: &StructDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&struct_decl.visibility, out);
    if struct_decl.is_data {
        out.push_str("data ");
    }
    out.push_str("struct ");
    format_ident(&struct_decl.name, out);
    format_generic_params(&struct_decl.generics, out);
    out.push_str(" {\n");
    for field in &struct_decl.fields {
        write_indent(indent + 1, out);
        format_ident(&field.name, out);
        out.push_str(": ");
        format_type(&field.ty, out);
        out.push_str(",\n");
    }
    write_indent(indent, out);
    out.push('}');
}

fn render_enum_signature(enum_decl: &EnumDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&enum_decl.visibility, out);
    out.push_str("enum ");
    format_ident(&enum_decl.name, out);
    format_generic_params(&enum_decl.generics, out);
    out.push_str(" {\n");
    for variant in &enum_decl.variants {
        write_indent(indent + 1, out);
        format_ident(&variant.name, out);
        match &variant.fields {
            VariantFields::Unit => {}
            VariantFields::Tuple(types) => {
                out.push('(');
                for (index, ty) in types.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    format_type(ty, out);
                }
                out.push(')');
            }
            VariantFields::Struct(fields) => {
                out.push_str(" {\n");
                for field in fields {
                    write_indent(indent + 2, out);
                    format_ident(&field.name, out);
                    out.push_str(": ");
                    format_type(&field.ty, out);
                    out.push_str(",\n");
                }
                write_indent(indent + 1, out);
                out.push('}');
            }
        }
        out.push_str(",\n");
    }
    write_indent(indent, out);
    out.push('}');
}

fn render_trait_signature(trait_decl: &TraitDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&trait_decl.visibility, out);
    out.push_str("trait ");
    format_ident(&trait_decl.name, out);
    format_generic_params(&trait_decl.generics, out);
    out.push_str(" {\n");
    for method in &trait_decl.methods {
        render_function_signature(method, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn render_impl_signature(impl_block: &ImplBlock, indent: usize, out: &mut String) -> bool {
    let public_methods = impl_block
        .methods
        .iter()
        .filter(|method| is_public(&method.visibility))
        .collect::<Vec<_>>();
    if public_methods.is_empty() {
        return false;
    }

    write_indent(indent, out);
    out.push_str("impl");
    format_generic_params(&impl_block.generics, out);
    out.push(' ');
    if let Some(trait_ty) = &impl_block.trait_ty {
        format_type(trait_ty, out);
        out.push_str(" for ");
    }
    format_type(&impl_block.target, out);
    format_where_clause(&impl_block.where_clause, indent, out);
    if impl_block.where_clause.is_empty() {
        out.push_str(" {\n");
    } else {
        write_indent(indent, out);
        out.push_str("{\n");
    }

    for (index, method) in public_methods.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        render_function_signature(method, indent + 1, false, out);
        out.push('\n');
    }

    write_indent(indent, out);
    out.push('}');
    true
}

fn render_extend_signature(extend_block: &ExtendBlock, indent: usize, out: &mut String) -> bool {
    let public_methods = extend_block
        .methods
        .iter()
        .filter(|method| is_public(&method.visibility))
        .collect::<Vec<_>>();
    if public_methods.is_empty() {
        return false;
    }

    write_indent(indent, out);
    out.push_str("extend ");
    format_type(&extend_block.target, out);
    out.push_str(" {\n");
    for (index, method) in public_methods.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        render_function_signature(method, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
    true
}

fn render_type_alias_signature(type_alias: &TypeAliasDecl, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&type_alias.visibility, out);
    if type_alias.is_opaque {
        out.push_str("opaque ");
    }
    out.push_str("type ");
    format_ident(&type_alias.name, out);
    format_generic_params(&type_alias.generics, out);
    out.push_str(" = ");
    format_type(&type_alias.ty, out);
}

fn render_extern_block_signature(extern_block: &ExternBlock, indent: usize, out: &mut String) {
    write_indent(indent, out);
    format_visibility(&extern_block.visibility, out);
    out.push_str("extern ");
    out.push('"');
    out.push_str(&extern_block.abi);
    out.push('"');
    out.push_str(" {\n");
    for function in &extern_block.functions {
        render_function_signature(function, indent + 1, false, out);
        out.push('\n');
    }
    write_indent(indent, out);
    out.push('}');
}

fn render_function_signature(
    function: &FunctionDecl,
    indent: usize,
    show_abi: bool,
    out: &mut String,
) {
    write_indent(indent, out);
    if show_abi && let Some(abi) = &function.abi {
        out.push_str("extern ");
        out.push('"');
        out.push_str(abi);
        out.push('"');
        out.push(' ');
    }
    format_visibility(&function.visibility, out);
    if function.is_unsafe {
        out.push_str("unsafe ");
    }
    if function.is_async {
        out.push_str("async ");
    }
    out.push_str("fn ");
    format_ident(&function.name, out);
    format_generic_params(&function.generics, out);
    out.push('(');
    for (index, param) in function.params.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        match param {
            Param::Regular { name, ty, .. } => {
                format_ident(name, out);
                out.push_str(": ");
                format_type(ty, out);
            }
            Param::Receiver { kind, .. } => match kind {
                ReceiverKind::ReadOnly => out.push_str("self"),
                ReceiverKind::Mutable => out.push_str("var self"),
                ReceiverKind::Move => out.push_str("move self"),
            },
        }
    }
    out.push(')');
    if let Some(ty) = &function.return_type {
        out.push_str(" -> ");
        format_type(ty, out);
    }
    format_where_clause(&function.where_clause, indent, out);
}

fn format_generic_params(params: &[GenericParam], out: &mut String) {
    if params.is_empty() {
        return;
    }

    out.push('[');
    for (index, param) in params.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        format_ident(&param.name, out);
        if !param.bounds.is_empty() {
            out.push_str(": ");
            for (bound_index, bound) in param.bounds.iter().enumerate() {
                if bound_index > 0 {
                    out.push_str(" + ");
                }
                format_path(bound, out);
            }
        }
    }
    out.push(']');
}

fn format_where_clause(predicates: &[WherePredicate], indent: usize, out: &mut String) {
    if predicates.is_empty() {
        return;
    }

    out.push('\n');
    write_indent(indent, out);
    out.push_str("where\n");
    for (index, predicate) in predicates.iter().enumerate() {
        write_indent(indent + 1, out);
        format_type(&predicate.target, out);
        out.push_str(": ");
        for (bound_index, bound) in predicate.bounds.iter().enumerate() {
            if bound_index > 0 {
                out.push_str(" + ");
            }
            format_path(bound, out);
        }
        if index + 1 != predicates.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
}

fn format_visibility(visibility: &Visibility, out: &mut String) {
    if is_public(visibility) {
        out.push_str("pub ");
    }
}

fn format_type(ty: &TypeExpr, out: &mut String) {
    match &ty.kind {
        TypeExprKind::Pointer { is_const, inner } => {
            out.push('*');
            if *is_const {
                out.push_str("const ");
            }
            format_type(inner, out);
        }
        TypeExprKind::Array { element, len } => {
            out.push('[');
            format_type(element, out);
            out.push_str("; ");
            out.push_str(len);
            out.push(']');
        }
        TypeExprKind::Named { path, args } => {
            format_path(path, out);
            if !args.is_empty() {
                out.push('[');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    format_type(arg, out);
                }
                out.push(']');
            }
        }
        TypeExprKind::Tuple(items) => {
            out.push('(');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                format_type(item, out);
            }
            if items.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        TypeExprKind::Callable { params, ret } => {
            out.push('(');
            for (index, param) in params.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                format_type(param, out);
            }
            out.push_str(") -> ");
            format_type(ret, out);
        }
    }
}

fn format_path(path: &AstPath, out: &mut String) {
    for (index, segment) in path.segments.iter().enumerate() {
        if index > 0 {
            out.push('.');
        }
        format_ident(segment, out);
    }
}

fn format_ident(name: &str, out: &mut String) {
    if needs_identifier_escape(name) {
        out.push('`');
        out.push_str(name);
        out.push('`');
    } else {
        out.push_str(name);
    }
}

fn needs_identifier_escape(name: &str) -> bool {
    lexer_is_keyword(name)
        || name.is_empty()
        || !name.chars().next().is_some_and(is_ident_start)
        || !name.chars().all(is_ident_continue)
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn write_indent(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public)
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

fn relative_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(normalize_display_path)
        .unwrap_or_else(|_| display_path(path))
}

fn normalize_display_path(path: &Path) -> String {
    display_path(&normalize_path(path))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}
