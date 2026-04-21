use std::collections::{BTreeMap, BTreeSet};
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
use serde_json::{Value as JsonValue, json};
use toml::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageManifest {
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestBuildProfile {
    Debug,
    Release,
}

impl ManifestBuildProfile {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "debug" => Some(Self::Debug),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
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
pub struct LibraryTargetManifest {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryTargetManifest {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileManifest {
    pub default: ManifestBuildProfile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectManifest {
    pub manifest_path: PathBuf,
    pub package: Option<PackageManifest>,
    pub workspace: Option<WorkspaceManifest>,
    pub references: ReferencesManifest,
    pub profile: Option<ProfileManifest>,
    pub lib: Option<LibraryTargetManifest>,
    pub bins: Vec<BinaryTargetManifest>,
}

pub const PROJECT_LOCKFILE_NAME: &str = "qlang.lock";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildTargetKind {
    Library,
    Binary,
    Source,
}

impl BuildTargetKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Library => "lib",
            Self::Binary => "bin",
            Self::Source => "source",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildTarget {
    pub kind: BuildTargetKind,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceBuildTargets {
    pub member_manifest_path: PathBuf,
    pub package_name: String,
    pub default_profile: Option<ManifestBuildProfile>,
    pub targets: Vec<BuildTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectLockRoot {
    manifest_path: PathBuf,
    default_profile: Option<ManifestBuildProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectLockPackage {
    manifest_path: PathBuf,
    package_name: String,
    selected: bool,
    default_profile: Option<ManifestBuildProfile>,
    dependencies: Vec<PathBuf>,
    targets: Vec<BuildTarget>,
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
    let profile = parse_profile(root, &manifest_path)?;
    let lib = parse_lib(root, &manifest_path)?;
    let bins = parse_bins(root, &manifest_path)?;

    if package.is_none() && workspace.is_none() {
        return Err(ProjectError::Parse {
            path: manifest_path.clone(),
            message: "`qlang.toml` requires `[package]` or `[workspace]`".to_owned(),
        });
    }

    if package.is_none() && (lib.is_some() || !bins.is_empty()) {
        return Err(ProjectError::Parse {
            path: manifest_path.clone(),
            message: "`[lib]` and `[[bin]]` require `[package]`".to_owned(),
        });
    }

    Ok(ProjectManifest {
        manifest_path,
        package,
        workspace,
        references,
        profile,
        lib,
        bins,
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

pub fn render_project_graph_resolved_json(
    manifest: &ProjectManifest,
) -> Result<String, ProjectError> {
    let rendered = serde_json::to_string_pretty(&project_graph_json(manifest))
        .expect("project graph json should serialize");
    Ok(format!("{rendered}\n"))
}

pub fn project_lockfile_path(manifest: &ProjectManifest) -> PathBuf {
    manifest_dir(manifest).join(PROJECT_LOCKFILE_NAME)
}

pub fn render_project_lockfile(manifest: &ProjectManifest) -> Result<String, ProjectError> {
    let rendered = serde_json::to_string_pretty(&project_lock_json(manifest)?)
        .expect("project lock json should serialize");
    Ok(format!("{rendered}\n"))
}

fn project_lock_json(manifest: &ProjectManifest) -> Result<JsonValue, ProjectError> {
    let root = manifest_dir(manifest);
    let lock_roots = project_lock_roots(manifest)?;
    let workspace_members = if manifest.workspace.is_some() {
        lock_roots
            .iter()
            .map(|root_package| relative_display_path(root, &root_package.manifest_path))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let packages = collect_project_lock_packages(manifest, &lock_roots)?;

    Ok(json!({
        "schema": "ql.project.lock.v1",
        "root": {
            "kind": if manifest.workspace.is_some() { "workspace" } else { "package" },
            "manifest_path": relative_display_path(root, &manifest.manifest_path),
        },
        "workspace_members": workspace_members,
        "packages": packages
            .iter()
            .map(|package| project_lock_package_json(root, package))
            .collect::<Vec<_>>(),
    }))
}

fn project_lock_roots(manifest: &ProjectManifest) -> Result<Vec<ProjectLockRoot>, ProjectError> {
    if manifest.workspace.is_none() {
        return Ok(vec![ProjectLockRoot {
            manifest_path: manifest.manifest_path.clone(),
            default_profile: manifest.profile.as_ref().map(|profile| profile.default),
        }]);
    }

    Ok(discover_workspace_build_targets(manifest)?
        .into_iter()
        .map(|member| ProjectLockRoot {
            manifest_path: member.member_manifest_path,
            default_profile: member.default_profile,
        })
        .collect())
}

fn collect_project_lock_packages(
    root_manifest: &ProjectManifest,
    roots: &[ProjectLockRoot],
) -> Result<Vec<ProjectLockPackage>, ProjectError> {
    let mut selected_profiles = BTreeMap::new();
    for root in roots {
        selected_profiles.insert(normalize_path(&root.manifest_path), root.default_profile);
    }

    let mut manifest_cache = BTreeMap::new();
    manifest_cache.insert(
        normalize_path(&root_manifest.manifest_path),
        root_manifest.clone(),
    );

    let mut visiting = Vec::new();
    let mut visited = BTreeSet::new();
    let mut packages = Vec::new();

    for root in roots {
        visit_project_lock_package(
            &normalize_path(&root.manifest_path),
            root_manifest,
            &selected_profiles,
            &mut manifest_cache,
            &mut visiting,
            &mut visited,
            &mut packages,
        )?;
    }

    Ok(packages)
}

fn visit_project_lock_package(
    manifest_path: &Path,
    root_manifest: &ProjectManifest,
    selected_profiles: &BTreeMap<PathBuf, Option<ManifestBuildProfile>>,
    manifest_cache: &mut BTreeMap<PathBuf, ProjectManifest>,
    visiting: &mut Vec<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
    packages: &mut Vec<ProjectLockPackage>,
) -> Result<(), ProjectError> {
    let manifest_path = normalize_path(manifest_path);
    if visited.contains(&manifest_path) {
        return Ok(());
    }

    if let Some(cycle_start) = visiting.iter().position(|path| path == &manifest_path) {
        let cycle_packages = visiting[cycle_start..]
            .iter()
            .map(|path| project_lock_cycle_label(manifest_cache, path))
            .collect::<Vec<_>>();
        return Err(ProjectError::Parse {
            path: root_manifest.manifest_path.clone(),
            message: format!(
                "local dependencies contain a cycle involving: {}",
                cycle_packages.join(", ")
            ),
        });
    }

    let manifest = load_cached_project_manifest(manifest_cache, &manifest_path)?;
    let package_name = package_name(&manifest)?.to_owned();
    let targets = discover_package_build_targets(&manifest)?;
    let references = load_reference_manifests(&manifest)?;
    let dependency_paths = references
        .iter()
        .map(|reference| normalize_path(&reference.manifest_path))
        .collect::<Vec<_>>();

    for reference in references {
        manifest_cache.insert(normalize_path(&reference.manifest_path), reference);
    }

    visiting.push(manifest_path.clone());
    for dependency_path in &dependency_paths {
        visit_project_lock_package(
            dependency_path,
            root_manifest,
            selected_profiles,
            manifest_cache,
            visiting,
            visited,
            packages,
        )?;
    }
    visiting.pop();
    visited.insert(manifest_path.clone());

    packages.push(ProjectLockPackage {
        selected: selected_profiles.contains_key(&manifest_path),
        default_profile: selected_profiles
            .get(&manifest_path)
            .copied()
            .flatten()
            .or_else(|| manifest.profile.as_ref().map(|profile| profile.default)),
        manifest_path,
        package_name,
        dependencies: dependency_paths,
        targets,
    });

    Ok(())
}

fn load_cached_project_manifest(
    manifest_cache: &mut BTreeMap<PathBuf, ProjectManifest>,
    manifest_path: &Path,
) -> Result<ProjectManifest, ProjectError> {
    let manifest_path = normalize_path(manifest_path);
    if let Some(manifest) = manifest_cache.get(&manifest_path) {
        return Ok(manifest.clone());
    }

    let manifest = load_project_manifest(&manifest_path)?;
    manifest_cache.insert(manifest_path, manifest.clone());
    Ok(manifest)
}

fn project_lock_cycle_label(
    manifest_cache: &BTreeMap<PathBuf, ProjectManifest>,
    manifest_path: &Path,
) -> String {
    manifest_cache
        .get(&normalize_path(manifest_path))
        .and_then(|manifest| {
            manifest
                .package
                .as_ref()
                .map(|package| package.name.clone())
        })
        .unwrap_or_else(|| normalize_display_path(manifest_path))
}

fn project_lock_package_json(root: &Path, package: &ProjectLockPackage) -> JsonValue {
    json!({
        "manifest_path": relative_display_path(root, &package.manifest_path),
        "package_name": package.package_name,
        "selected": package.selected,
        "default_profile": package.default_profile.map(|profile| profile.as_str()),
        "dependencies": package
            .dependencies
            .iter()
            .map(|path| relative_display_path(root, path))
            .collect::<Vec<_>>(),
        "targets": package
            .targets
            .iter()
            .map(|target| json!({
                "kind": target.kind.as_str(),
                "path": relative_display_path(root, &target.path),
            }))
            .collect::<Vec<_>>(),
    })
}

fn project_graph_json(manifest: &ProjectManifest) -> JsonValue {
    let manifest_dir = manifest_dir(manifest);
    let interface = if manifest.package.is_some() {
        let interface_path =
            default_interface_path(manifest).expect("package manifests have a default interface");
        project_graph_interface_summary_json(manifest_dir, manifest, &interface_path)
    } else {
        JsonValue::Null
    };
    let reference_interfaces = if manifest.package.is_some() {
        project_graph_reference_interface_summaries_json(manifest_dir, manifest)
    } else {
        Vec::new()
    };
    let workspace_packages = if manifest.package.is_none() && manifest.workspace.is_some() {
        project_graph_workspace_packages_json(manifest)
    } else {
        Vec::new()
    };

    json!({
        "schema": "ql.project.graph.v1",
        "manifest_path": display_path(&manifest.manifest_path),
        "package_name": manifest.package.as_ref().map(|package| package.name.as_str()),
        "workspace_members": manifest.workspace.as_ref().map(|workspace| workspace.members.clone()).unwrap_or_default(),
        "references": manifest.references.packages.clone(),
        "interface": interface,
        "reference_interfaces": reference_interfaces,
        "workspace_packages": workspace_packages,
    })
}

fn project_graph_workspace_packages_json(manifest: &ProjectManifest) -> Vec<JsonValue> {
    let root = manifest_dir(manifest);
    let mut members = Vec::new();

    for member in &manifest
        .workspace
        .as_ref()
        .expect("workspace exists")
        .members
    {
        let member_path = root.join(member);
        let member_manifest = match load_project_manifest(&member_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                members.push(project_graph_workspace_member_error_json(
                    root,
                    member,
                    &project_manifest_path(&member_path),
                    &error,
                ));
                continue;
            }
        };
        let package = match package_name(&member_manifest) {
            Ok(package) => package.to_owned(),
            Err(error) => {
                members.push(project_graph_workspace_member_error_json(
                    root,
                    member,
                    &member_manifest.manifest_path,
                    &error,
                ));
                continue;
            }
        };
        let interface_path = match default_interface_path(&member_manifest) {
            Ok(path) => path,
            Err(error) => {
                members.push(project_graph_workspace_member_error_json(
                    root,
                    member,
                    &member_manifest.manifest_path,
                    &error,
                ));
                continue;
            }
        };

        members.push(json!({
            "member": member,
            "manifest_path": relative_display_path(root, &member_manifest.manifest_path),
            "package_name": package,
            "member_status": JsonValue::Null,
            "member_error": JsonValue::Null,
            "interface": project_graph_interface_summary_json(root, &member_manifest, &interface_path),
            "references": member_manifest.references.packages.clone(),
            "reference_interfaces": project_graph_reference_interface_summaries_json(root, &member_manifest),
        }));
    }

    members
}

fn project_graph_workspace_member_error_json(
    root: &Path,
    member: &str,
    manifest_path: &Path,
    error: &ProjectError,
) -> JsonValue {
    json!({
        "member": member,
        "manifest_path": relative_display_path(root, manifest_path),
        "package_name": JsonValue::Null,
        "member_status": project_graph_member_status(error),
        "member_error": project_graph_error_display_relative(root, error),
        "interface": JsonValue::Null,
        "references": Vec::<String>::new(),
        "reference_interfaces": Vec::<JsonValue>::new(),
    })
}

fn project_graph_interface_summary_json(
    root: &Path,
    manifest: &ProjectManifest,
    interface_path: &Path,
) -> JsonValue {
    let status = interface_artifact_status(manifest, interface_path);
    let stale_reasons = if status == InterfaceArtifactStatus::Stale {
        interface_artifact_stale_reasons(manifest, interface_path)
    } else {
        Vec::new()
    };

    json!({
        "path": relative_display_path(root, interface_path),
        "status": status.label(),
        "detail": interface_artifact_status_detail(interface_path, status),
        "stale_reasons": project_graph_stale_reasons_json(root, &stale_reasons),
    })
}

fn project_graph_reference_interface_summaries_json(
    root: &Path,
    manifest: &ProjectManifest,
) -> Vec<JsonValue> {
    let manifest_dir = manifest_dir(manifest);
    let mut references = Vec::new();

    for reference in &manifest.references.packages {
        let reference_manifest_path = project_manifest_path(&manifest_dir.join(reference));
        match load_project_manifest(&manifest_dir.join(reference)) {
            Ok(reference_manifest) => match default_interface_path(&reference_manifest) {
                Ok(interface_path) => {
                    let status = interface_artifact_status(&reference_manifest, &interface_path);
                    let stale_reasons = if status == InterfaceArtifactStatus::Stale {
                        interface_artifact_stale_reasons(&reference_manifest, &interface_path)
                    } else {
                        Vec::new()
                    };
                    let transitive_reference_failures =
                        summarize_transitive_reference_failures(root, &reference_manifest);

                    references.push(json!({
                        "reference": reference,
                        "manifest_path": relative_display_path(root, &reference_manifest_path),
                        "package_name": reference_manifest.package.as_ref().map(|package| package.name.as_str()),
                        "path": relative_display_path(root, &interface_path),
                        "status": status.label(),
                        "detail": interface_artifact_status_detail(&interface_path, status),
                        "stale_reasons": project_graph_stale_reasons_json(root, &stale_reasons),
                        "transitive_reference_failures": project_graph_transitive_reference_failures_json(
                            root,
                            &transitive_reference_failures,
                        ),
                    }));
                }
                Err(error) => {
                    references.push(json!({
                        "reference": reference,
                        "manifest_path": relative_display_path(root, &reference_manifest_path),
                        "package_name": JsonValue::Null,
                        "path": JsonValue::Null,
                        "status": "unresolved-package",
                        "detail": project_graph_error_display_relative(root, &error),
                        "stale_reasons": Vec::<JsonValue>::new(),
                        "transitive_reference_failures": project_graph_transitive_reference_failures_json(
                            root,
                            &TransitiveReferenceFailureSummary {
                                count: 0,
                                first_failure: None,
                            },
                        ),
                    }));
                }
            },
            Err(error) => {
                references.push(json!({
                    "reference": reference,
                    "manifest_path": relative_display_path(root, &reference_manifest_path),
                    "package_name": JsonValue::Null,
                    "path": JsonValue::Null,
                    "status": "unresolved-manifest",
                    "detail": project_graph_error_display_relative(root, &error),
                    "stale_reasons": Vec::<JsonValue>::new(),
                    "transitive_reference_failures": project_graph_transitive_reference_failures_json(
                        root,
                        &TransitiveReferenceFailureSummary {
                            count: 0,
                            first_failure: None,
                        },
                    ),
                }));
            }
        }
    }

    references
}

fn project_graph_transitive_reference_failures_json(
    root: &Path,
    summary: &TransitiveReferenceFailureSummary,
) -> JsonValue {
    json!({
        "count": summary.count,
        "first_failure": summary
            .first_failure
            .as_ref()
            .map(|failure| project_graph_transitive_reference_failure_json(root, failure)),
    })
}

fn project_graph_transitive_reference_failure_json(
    root: &Path,
    failure: &TransitiveReferenceFailure,
) -> JsonValue {
    json!({
        "manifest_path": relative_display_path(root, &failure.manifest_path),
        "interface_path": failure.interface_path.as_ref().map(|path| relative_display_path(root, path)),
        "status": failure.status,
        "detail": failure.detail,
        "stale_reasons": project_graph_stale_reasons_json(root, &failure.stale_reasons),
    })
}

fn project_graph_stale_reasons_json(
    root: &Path,
    stale_reasons: &[InterfaceArtifactStaleReason],
) -> Vec<JsonValue> {
    stale_reasons
        .iter()
        .map(|reason| {
            json!({
                "kind": reason.label(),
                "path": relative_display_path(root, reason.path()),
            })
        })
        .collect()
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
    output.push_str(&format!(
        "    member_status: {}\n",
        project_graph_member_status(error)
    ));
    output.push_str(&format!(
        "    member_error: {}\n",
        project_graph_error_display_relative(root, error)
    ));
}

fn project_graph_error_display_relative(root: &Path, error: &ProjectError) -> String {
    project_graph_error_display_with(error, |path| relative_display_path(root, path))
}

fn project_graph_error_display_with<F>(error: &ProjectError, mut render_path: F) -> String
where
    F: FnMut(&Path) -> String,
{
    if let Some(path) = project_error_missing_package_name_manifest_path(error) {
        return format!(
            "manifest `{}` does not declare `[package].name`",
            render_path(path)
        );
    }
    match error {
        ProjectError::ManifestNotFound { start } => format!(
            "could not find `qlang.toml` starting from `{}`",
            render_path(start)
        ),
        ProjectError::PackageNotDefined { path } => format!(
            "manifest `{}` does not declare `[package].name`",
            render_path(path)
        ),
        ProjectError::PackageSourceRootNotFound { path } => format!(
            "package source directory `{}` does not exist",
            render_path(path)
        ),
        ProjectError::Read { path, error } => {
            format!("failed to read manifest `{}`: {error}", render_path(path))
        }
        ProjectError::Parse { path, message } => {
            format!("invalid manifest `{}`: {message}", render_path(path))
        }
    }
}

fn project_error_missing_package_name_manifest_path(error: &ProjectError) -> Option<&Path> {
    match error {
        ProjectError::PackageNotDefined { path } => Some(path.as_path()),
        ProjectError::Parse { path, message } if message == "`[package].name` must be present" => {
            Some(path.as_path())
        }
        _ => None,
    }
}

fn project_graph_member_status(error: &ProjectError) -> &'static str {
    if project_error_missing_package_name_manifest_path(error).is_some() {
        "unresolved-package"
    } else {
        "unresolved-manifest"
    }
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
                        summarize_transitive_reference_failures(root, &reference_manifest);
                    if transitive_reference_failures.count > 0 {
                        output.push_str(&format!(
                            "{indent}    transitive_reference_failures: {}\n",
                            transitive_reference_failures.count
                        ));
                        if let Some(first_failure) = &transitive_reference_failures.first_failure {
                            output.push_str(&format!(
                                "{indent}    first_transitive_failure_manifest: {}\n",
                                relative_display_path(root, &first_failure.manifest_path)
                            ));
                            if let Some(interface_path) = &first_failure.interface_path {
                                output.push_str(&format!(
                                    "{indent}    first_transitive_failure_path: {}\n",
                                    relative_display_path(root, interface_path)
                                ));
                            }
                            output.push_str(&format!(
                                "{indent}    first_transitive_failure_status: {}\n",
                                first_failure.status
                            ));
                            if let Some(detail) = &first_failure.detail {
                                output.push_str(&format!(
                                    "{indent}    first_transitive_failure_detail: {detail}\n"
                                ));
                            }
                            if !first_failure.stale_reasons.is_empty() {
                                output.push_str(&format!(
                                    "{indent}    first_transitive_failure_stale_reasons:\n"
                                ));
                                for reason in &first_failure.stale_reasons {
                                    output.push_str(&format!(
                                        "{indent}      - {}: {}\n",
                                        reason.label(),
                                        relative_display_path(root, reason.path())
                                    ));
                                }
                            }
                        }
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
                    output.push_str(&format!(
                        "{indent}    detail: {}\n",
                        project_graph_error_display_relative(root, &error)
                    ));
                }
            },
            Err(error) => {
                output.push_str(&format!("{indent}    package: <unresolved>\n"));
                output.push_str(&format!("{indent}    path: <unresolved>\n"));
                output.push_str(&format!("{indent}    status: unresolved-manifest\n"));
                output.push_str(&format!(
                    "{indent}    detail: {}\n",
                    project_graph_error_display_relative(root, &error)
                ));
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TransitiveReferenceFailure {
    manifest_path: PathBuf,
    interface_path: Option<PathBuf>,
    status: &'static str,
    detail: Option<String>,
    stale_reasons: Vec<InterfaceArtifactStaleReason>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TransitiveReferenceFailureSummary {
    count: usize,
    first_failure: Option<TransitiveReferenceFailure>,
}

fn summarize_transitive_reference_failures(
    root: &Path,
    manifest: &ProjectManifest,
) -> TransitiveReferenceFailureSummary {
    let mut visited = BTreeSet::new();
    summarize_reference_failures_recursive(root, manifest, &mut visited)
}

fn summarize_reference_failures_recursive(
    root: &Path,
    manifest: &ProjectManifest,
    visited: &mut BTreeSet<PathBuf>,
) -> TransitiveReferenceFailureSummary {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return TransitiveReferenceFailureSummary {
            count: 0,
            first_failure: None,
        };
    }

    let manifest_dir = manifest_dir(manifest);
    let mut summary = TransitiveReferenceFailureSummary {
        count: 0,
        first_failure: None,
    };
    for reference in &manifest.references.packages {
        let reference_manifest_path = project_manifest_path(&manifest_dir.join(reference));
        match load_project_manifest(&manifest_dir.join(reference)) {
            Ok(reference_manifest) => {
                let interface_path = default_interface_path(&reference_manifest);
                match interface_path {
                    Ok(interface_path) => {
                        let status =
                            interface_artifact_status(&reference_manifest, &interface_path);
                        if status != InterfaceArtifactStatus::Valid {
                            summary.count += 1;
                            record_first_transitive_reference_failure(
                                &mut summary,
                                reference_manifest.manifest_path.clone(),
                                Some(interface_path.clone()),
                                status.label(),
                                interface_artifact_status_detail(&interface_path, status),
                                if status == InterfaceArtifactStatus::Stale {
                                    interface_artifact_stale_reasons(
                                        &reference_manifest,
                                        &interface_path,
                                    )
                                } else {
                                    Vec::new()
                                },
                            );
                        }
                    }
                    Err(error) => {
                        summary.count += 1;
                        record_first_transitive_reference_failure(
                            &mut summary,
                            reference_manifest.manifest_path.clone(),
                            None,
                            "unresolved-package",
                            Some(project_graph_error_display_relative(root, &error)),
                            Vec::new(),
                        );
                    }
                }
                let nested_summary =
                    summarize_reference_failures_recursive(root, &reference_manifest, visited);
                summary.count += nested_summary.count;
                if summary.first_failure.is_none() {
                    summary.first_failure = nested_summary.first_failure;
                }
            }
            Err(error) => {
                summary.count += 1;
                record_first_transitive_reference_failure(
                    &mut summary,
                    reference_manifest_path,
                    None,
                    "unresolved-manifest",
                    Some(project_graph_error_display_relative(root, &error)),
                    Vec::new(),
                );
            }
        }
    }

    summary
}

fn record_first_transitive_reference_failure(
    summary: &mut TransitiveReferenceFailureSummary,
    manifest_path: PathBuf,
    interface_path: Option<PathBuf>,
    status: &'static str,
    detail: Option<String>,
    stale_reasons: Vec<InterfaceArtifactStaleReason>,
) {
    summary
        .first_failure
        .get_or_insert(TransitiveReferenceFailure {
            manifest_path,
            interface_path,
            status,
            detail,
            stale_reasons,
        });
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

pub fn discover_package_build_targets(
    manifest: &ProjectManifest,
) -> Result<Vec<BuildTarget>, ProjectError> {
    let _ = package_name(manifest)?;
    let source_root = package_source_root(manifest)?;
    if !source_root.is_dir() {
        return Err(ProjectError::PackageSourceRootNotFound { path: source_root });
    }

    let mut targets = Vec::new();
    if let Some(lib) = &manifest.lib {
        targets.push(BuildTarget {
            kind: BuildTargetKind::Library,
            path: resolve_declared_target_path(manifest, &lib.path, "`[lib].path`")?,
        });
    } else {
        let lib_path = source_root.join("lib.ql");
        if lib_path.is_file() {
            targets.push(BuildTarget {
                kind: BuildTargetKind::Library,
                path: lib_path,
            });
        }
    }

    if !manifest.bins.is_empty() {
        let mut seen = BTreeSet::new();
        for bin in &manifest.bins {
            let path = resolve_declared_target_path(manifest, &bin.path, "`[[bin]].path`")?;
            if seen.insert(path.clone()) {
                targets.push(BuildTarget {
                    kind: BuildTargetKind::Binary,
                    path,
                });
            }
        }
    } else {
        let main_path = source_root.join("main.ql");
        if main_path.is_file() {
            targets.push(BuildTarget {
                kind: BuildTargetKind::Binary,
                path: main_path,
            });
        }

        let bin_root = source_root.join("bin");
        if bin_root.is_dir() {
            let mut bin_targets = Vec::new();
            collect_bin_targets_recursive(&bin_root, &mut bin_targets)?;
            bin_targets.sort_by(|left, right| left.path.cmp(&right.path));
            targets.extend(bin_targets);
        }
    }

    if targets.is_empty() {
        let sources = collect_package_sources(manifest)?;
        if sources.len() == 1 {
            targets.push(BuildTarget {
                kind: BuildTargetKind::Source,
                path: sources[0].clone(),
            });
        }
    }

    Ok(targets)
}

fn resolve_declared_target_path(
    manifest: &ProjectManifest,
    relative_path: &str,
    field_name: &str,
) -> Result<PathBuf, ProjectError> {
    let raw_path = Path::new(relative_path);
    if raw_path.is_absolute() {
        return Err(parse_error(
            &manifest.manifest_path,
            format!("{field_name} must be a package-relative `.ql` path under `src/`"),
        ));
    }

    let package_root = normalize_path(manifest_dir(manifest));
    let source_root = normalize_path(&package_root.join("src"));
    let target_path = normalize_path(&package_root.join(raw_path));
    if !target_path.starts_with(&source_root) {
        return Err(parse_error(
            &manifest.manifest_path,
            format!("{field_name} must stay under `src/`"),
        ));
    }

    if target_path.extension().and_then(|ext| ext.to_str()) != Some("ql") {
        return Err(parse_error(
            &manifest.manifest_path,
            format!("{field_name} must point to a `.ql` source file under `src/`"),
        ));
    }

    if !target_path.is_file() {
        return Err(parse_error(
            &manifest.manifest_path,
            format!(
                "{field_name} declares missing target `{}`",
                normalize_display_path(&target_path)
            ),
        ));
    }

    Ok(target_path)
}

pub fn discover_workspace_build_targets(
    manifest: &ProjectManifest,
) -> Result<Vec<WorkspaceBuildTargets>, ProjectError> {
    if manifest.workspace.is_none() {
        return Ok(vec![WorkspaceBuildTargets {
            member_manifest_path: manifest.manifest_path.clone(),
            package_name: package_name(manifest)?.to_owned(),
            default_profile: manifest.profile.as_ref().map(|profile| profile.default),
            targets: discover_package_build_targets(manifest)?,
        }]);
    }

    let workspace_default_profile = manifest.profile.as_ref().map(|profile| profile.default);
    let member_manifests = ordered_workspace_member_manifests(manifest)?;
    let mut members = Vec::with_capacity(member_manifests.len());
    for member_manifest in &member_manifests {
        members.push(WorkspaceBuildTargets {
            member_manifest_path: member_manifest.manifest_path.clone(),
            package_name: package_name(member_manifest)?.to_owned(),
            default_profile: member_manifest
                .profile
                .as_ref()
                .map(|profile| profile.default)
                .or(workspace_default_profile),
            targets: discover_package_build_targets(member_manifest)?,
        });
    }
    Ok(members)
}

fn ordered_workspace_member_manifests(
    workspace_manifest: &ProjectManifest,
) -> Result<Vec<ProjectManifest>, ProjectError> {
    let manifest_dir = manifest_dir(workspace_manifest);
    let mut member_manifests = Vec::new();
    for member in &workspace_manifest
        .workspace
        .as_ref()
        .expect("workspace presence checked above")
        .members
    {
        let member_path = manifest_dir.join(member);
        member_manifests.push(load_project_manifest(&member_path)?);
    }

    let ordered_member_indexes =
        workspace_member_dependency_order(workspace_manifest, &member_manifests)?;
    Ok(ordered_member_indexes
        .into_iter()
        .map(|index| member_manifests[index].clone())
        .collect())
}

fn workspace_member_dependency_order(
    workspace_manifest: &ProjectManifest,
    member_manifests: &[ProjectManifest],
) -> Result<Vec<usize>, ProjectError> {
    let mut member_indexes = std::collections::HashMap::with_capacity(member_manifests.len());
    for (index, manifest) in member_manifests.iter().enumerate() {
        member_indexes.insert(normalize_path(&manifest.manifest_path), index);
    }

    let mut indegrees = vec![0usize; member_manifests.len()];
    let mut dependents = vec![Vec::new(); member_manifests.len()];

    for (member_index, manifest) in member_manifests.iter().enumerate() {
        let member_dir = manifest_dir(manifest);
        let mut local_dependencies = BTreeSet::new();
        for reference in &manifest.references.packages {
            let dependency_manifest_path =
                normalize_path(&project_manifest_path(&member_dir.join(reference)));
            let Some(&dependency_index) = member_indexes.get(&dependency_manifest_path) else {
                continue;
            };
            if local_dependencies.insert(dependency_index) {
                indegrees[member_index] += 1;
                dependents[dependency_index].push(member_index);
            }
        }
    }

    let mut ordered = Vec::with_capacity(member_manifests.len());
    let mut processed = vec![false; member_manifests.len()];
    while let Some(next_index) =
        (0..member_manifests.len()).find(|&index| !processed[index] && indegrees[index] == 0)
    {
        processed[next_index] = true;
        ordered.push(next_index);
        for &dependent_index in &dependents[next_index] {
            indegrees[dependent_index] -= 1;
        }
    }

    if ordered.len() == member_manifests.len() {
        return Ok(ordered);
    }

    let cycle_packages = member_manifests
        .iter()
        .enumerate()
        .filter(|(index, _)| !processed[*index])
        .map(|(_, manifest)| {
            manifest
                .package
                .as_ref()
                .map(|package| package.name.clone())
                .unwrap_or_else(|| normalize_display_path(&manifest.manifest_path))
        })
        .collect::<Vec<_>>();
    Err(ProjectError::Parse {
        path: workspace_manifest.manifest_path.clone(),
        message: format!(
            "workspace member local dependencies contain a cycle involving: {}",
            cycle_packages.join(", ")
        ),
    })
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

pub fn render_manifest_with_added_local_dependency(
    source: &str,
    dependency_name: &str,
    dependency_path: &str,
) -> Result<String, String> {
    let mut value = toml::from_str::<Value>(source)
        .map_err(|error| format!("failed to parse package manifest: {error}"))?;
    let Some(root) = value.as_table_mut() else {
        return Err("package manifest must be a TOML table".to_owned());
    };
    if root.get("package").and_then(Value::as_table).is_none() {
        return Err("package manifest must declare `[package]`".to_owned());
    }

    let dependencies = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            "package manifest must declare `[dependencies]` as a TOML table".to_owned()
        })?;
    if dependencies.contains_key(dependency_name) {
        return Err(format!(
            "package manifest already declares local dependency `{dependency_name}`"
        ));
    }
    dependencies.insert(
        dependency_name.to_owned(),
        Value::String(dependency_path.to_owned()),
    );

    let mut rendered = toml::to_string(&value)
        .map_err(|error| format!("failed to render package manifest: {error}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

pub fn render_manifest_with_added_binary_target(
    source: &str,
    preserved_binary_paths: &[String],
    binary_path: &str,
) -> Result<String, String> {
    validate_manifest_binary_target_path(binary_path)?;

    let mut value = toml::from_str::<Value>(source)
        .map_err(|error| format!("failed to parse package manifest: {error}"))?;
    let Some(root) = value.as_table_mut() else {
        return Err("package manifest must be a TOML table".to_owned());
    };
    if root.get("package").and_then(Value::as_table).is_none() {
        return Err("package manifest must declare `[package]`".to_owned());
    }

    let bins = root
        .entry("bin")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| {
            "package manifest must declare `[[bin]]` as an array of tables".to_owned()
        })?;

    let mut declared_paths = bins
        .iter()
        .map(manifest_binary_target_path)
        .collect::<Result<BTreeSet<_>, _>>()?;
    if declared_paths.contains(binary_path) {
        return Err(format!(
            "package manifest already declares binary target `{binary_path}`"
        ));
    }

    if bins.is_empty() {
        for preserved_path in preserved_binary_paths {
            validate_manifest_binary_target_path(preserved_path)?;
            if declared_paths.insert(preserved_path.clone()) {
                bins.push(manifest_binary_target_table(preserved_path));
            }
        }
    }

    bins.push(manifest_binary_target_table(binary_path));

    let mut rendered = toml::to_string(&value)
        .map_err(|error| format!("failed to render package manifest: {error}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

pub fn render_manifest_with_removed_local_dependency(
    source: &str,
    dependency_name: &str,
    dependency_path: &str,
) -> Result<String, String> {
    let mut value = toml::from_str::<Value>(source)
        .map_err(|error| format!("failed to parse package manifest: {error}"))?;
    let Some(root) = value.as_table_mut() else {
        return Err("package manifest must be a TOML table".to_owned());
    };
    if root.get("package").and_then(Value::as_table).is_none() {
        return Err("package manifest must declare `[package]`".to_owned());
    }

    let dependency_path = normalize_path(Path::new(dependency_path));
    let mut removed = false;

    if let Some(dependencies) = root.get_mut("dependencies") {
        let dependencies = dependencies.as_table_mut().ok_or_else(|| {
            "package manifest must declare `[dependencies]` as a TOML table".to_owned()
        })?;
        let removed_keys = dependencies
            .iter()
            .filter_map(|(key, value)| {
                dependency_value_matches_path(value, &dependency_path).then_some(key.clone())
            })
            .collect::<Vec<_>>();
        for key in removed_keys {
            removed |= dependencies.remove(&key).is_some();
        }
        if dependencies.is_empty() {
            root.remove("dependencies");
        }
    }

    if let Some(references) = root.get_mut("references") {
        let references = references.as_table_mut().ok_or_else(|| {
            "package manifest must declare `[references]` as a TOML table".to_owned()
        })?;
        if let Some(packages) = references.get_mut("packages") {
            let packages = packages.as_array_mut().ok_or_else(|| {
                "package manifest must declare `[references].packages` as an array".to_owned()
            })?;
            let original_len = packages.len();
            packages.retain(|value| !reference_value_matches_path(value, &dependency_path));
            removed |= packages.len() != original_len;
            if packages.is_empty() {
                references.remove("packages");
            }
        }
        if references.is_empty() {
            root.remove("references");
        }
    }

    if !removed {
        return Err(format!(
            "package manifest does not declare local dependency `{dependency_name}`"
        ));
    }

    let mut rendered = toml::to_string(&value)
        .map_err(|error| format!("failed to render package manifest: {error}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn dependency_value_matches_path(value: &Value, dependency_path: &Path) -> bool {
    match value {
        Value::String(path) => normalize_path(Path::new(path)) == dependency_path,
        Value::Table(table) => table
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| normalize_path(Path::new(path)) == dependency_path),
        _ => false,
    }
}

fn validate_manifest_binary_target_path(binary_path: &str) -> Result<(), String> {
    let binary_path = binary_path.trim();
    if binary_path.is_empty() {
        return Err("binary target path must not be empty".to_owned());
    }

    let binary_path = Path::new(binary_path);
    if binary_path.is_absolute() {
        return Err("binary target path must stay under `src/`".to_owned());
    }
    if binary_path.extension().and_then(|ext| ext.to_str()) != Some("ql") {
        return Err("binary target path must point to a `.ql` source file under `src/`".to_owned());
    }

    let normalized = normalize_path(binary_path);
    if !normalized.starts_with("src") {
        return Err("binary target path must stay under `src/`".to_owned());
    }

    Ok(())
}

fn manifest_binary_target_path(value: &Value) -> Result<String, String> {
    let Some(table) = value.as_table() else {
        return Err("package manifest must declare `[[bin]]` as TOML tables".to_owned());
    };
    if table.keys().any(|key| key != "path") {
        return Err("package manifest `[[bin]]` entries currently only support `path`".to_owned());
    }
    let Some(path) = table.get("path").and_then(Value::as_str) else {
        return Err("package manifest `[[bin]].path` must be a string".to_owned());
    };
    Ok(path.to_owned())
}

fn manifest_binary_target_table(binary_path: &str) -> Value {
    let mut table = toml::Table::new();
    table.insert("path".to_owned(), Value::String(binary_path.to_owned()));
    Value::Table(table)
}

fn reference_value_matches_path(value: &Value, dependency_path: &Path) -> bool {
    value
        .as_str()
        .is_some_and(|path| normalize_path(Path::new(path)) == dependency_path)
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

fn collect_bin_targets_recursive(
    path: &Path,
    targets: &mut Vec<BuildTarget>,
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
            collect_bin_targets_recursive(&entry_path, targets)?;
        } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("ql") {
            targets.push(BuildTarget {
                kind: BuildTargetKind::Binary,
                path: entry_path,
            });
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

fn parse_profile(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Option<ProfileManifest>, ProjectError> {
    let Some(profile) = root.get("profile") else {
        return Ok(None);
    };
    let Some(profile) = profile.as_table() else {
        return Err(parse_error(
            manifest_path,
            "`[profile]` must be a TOML table",
        ));
    };
    if profile.keys().any(|key| key != "default") {
        return Err(parse_error(
            manifest_path,
            "`[profile]` currently only supports `default = \"debug|release\"`",
        ));
    }

    let Some(default) = profile.get("default") else {
        return Err(parse_error(
            manifest_path,
            "`[profile].default` must be present",
        ));
    };
    let Some(default) = default.as_str() else {
        return Err(parse_error(
            manifest_path,
            "`[profile].default` must be a string",
        ));
    };
    let Some(default) = ManifestBuildProfile::parse(default) else {
        return Err(parse_error(
            manifest_path,
            "`[profile].default` must be `debug` or `release`",
        ));
    };

    Ok(Some(ProfileManifest { default }))
}

fn parse_lib(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Option<LibraryTargetManifest>, ProjectError> {
    let Some(lib) = root.get("lib") else {
        return Ok(None);
    };
    let Some(lib) = lib.as_table() else {
        return Err(parse_error(manifest_path, "`[lib]` must be a TOML table"));
    };
    let path = parse_declared_target_path_field(lib, manifest_path, "`[lib]`")?;
    Ok(Some(LibraryTargetManifest { path }))
}

fn parse_bins(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Vec<BinaryTargetManifest>, ProjectError> {
    let Some(bins) = root.get("bin") else {
        return Ok(Vec::new());
    };
    let Some(bins) = bins.as_array() else {
        return Err(parse_error(
            manifest_path,
            "`[[bin]]` must be an array of TOML tables",
        ));
    };

    let mut parsed = Vec::with_capacity(bins.len());
    for bin in bins {
        let Some(bin) = bin.as_table() else {
            return Err(parse_error(
                manifest_path,
                "`[[bin]]` entries must be TOML tables",
            ));
        };
        let path = parse_declared_target_path_field(bin, manifest_path, "`[[bin]]`")?;
        parsed.push(BinaryTargetManifest { path });
    }
    Ok(parsed)
}

fn parse_declared_target_path_field(
    table: &toml::Table,
    manifest_path: &Path,
    table_name: &str,
) -> Result<String, ProjectError> {
    if table.keys().any(|key| key != "path") {
        return Err(parse_error(
            manifest_path,
            format!("{table_name} currently only supports `path = \"src/...\"`"),
        ));
    }

    let Some(path) = table.get("path") else {
        return Err(parse_error(
            manifest_path,
            format!("{table_name}.path must be present"),
        ));
    };
    let Some(path) = path.as_str() else {
        return Err(parse_error(
            manifest_path,
            format!("{table_name}.path must be a string"),
        ));
    };
    if path.trim().is_empty() {
        return Err(parse_error(
            manifest_path,
            format!("{table_name}.path must not be empty"),
        ));
    }

    Ok(path.to_owned())
}

fn parse_references(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<ReferencesManifest, ProjectError> {
    let mut packages = parse_legacy_reference_paths(root, manifest_path)?;
    packages.extend(parse_dependency_paths(root, manifest_path)?);
    packages = dedup_manifest_paths(packages);
    Ok(ReferencesManifest { packages })
}

fn parse_legacy_reference_paths(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Vec<String>, ProjectError> {
    let Some(references) = root.get("references") else {
        return Ok(Vec::new());
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
    Ok(packages)
}

fn parse_dependency_paths(
    root: &toml::Table,
    manifest_path: &Path,
) -> Result<Vec<String>, ProjectError> {
    let Some(dependencies) = root.get("dependencies") else {
        return Ok(Vec::new());
    };
    let Some(dependencies) = dependencies.as_table() else {
        return Err(parse_error(
            manifest_path,
            "`[dependencies]` must be a TOML table",
        ));
    };

    let mut packages = Vec::with_capacity(dependencies.len());
    for (dependency_name, value) in dependencies {
        let field_name = format!("`[dependencies].{dependency_name}`");
        let path = parse_dependency_path_value(value, manifest_path, &field_name)?;
        packages.push(path);
    }
    Ok(packages)
}

fn parse_dependency_path_value(
    value: &Value,
    manifest_path: &Path,
    field_name: &str,
) -> Result<String, ProjectError> {
    if let Some(path) = value.as_str() {
        return Ok(path.to_owned());
    }

    let Some(table) = value.as_table() else {
        return Err(parse_error(
            manifest_path,
            format!(
                "{field_name} currently only supports local path dependencies written as a string or `{{ path = \"...\" }}`"
            ),
        ));
    };

    if table.keys().any(|key| key != "path") {
        return Err(parse_error(
            manifest_path,
            format!(
                "{field_name} currently only supports local path dependencies written as a string or `{{ path = \"...\" }}`"
            ),
        ));
    }

    let Some(path) = table.get("path") else {
        return Err(parse_error(
            manifest_path,
            format!(
                "{field_name} currently only supports local path dependencies written as a string or `{{ path = \"...\" }}`"
            ),
        ));
    };
    let Some(path) = path.as_str() else {
        return Err(parse_error(
            manifest_path,
            format!("{field_name}.path must be a string"),
        ));
    };
    Ok(path.to_owned())
}

fn dedup_manifest_paths(paths: Vec<String>) -> Vec<String> {
    let mut unique = Vec::with_capacity(paths.len());
    for path in paths {
        if !unique.contains(&path) {
            unique.push(path);
        }
    }
    unique
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
