use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_project::{
    ProjectError, discover_workspace_build_targets, load_project_manifest, load_reference_manifests,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn load_reference_manifests_accepts_local_dependencies_and_legacy_references() {
    let temp = TempDir::new("ql-project-manifest-dependencies");
    let app_root = temp.path().join("app");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../legacy", "../dep"]

[dependencies]
dep = { path = "../dep" }
extra = "../extra"
"#,
    );
    temp.write(
        "legacy/qlang.toml",
        r#"
[package]
name = "legacy"
"#,
    );
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "extra/qlang.toml",
        r#"
[package]
name = "extra"
"#,
    );

    let manifest = load_project_manifest(&app_root).expect("load app manifest");
    assert_eq!(
        manifest.references.packages,
        vec![
            "../legacy".to_owned(),
            "../dep".to_owned(),
            "../extra".to_owned(),
        ],
        "new dependency syntax should normalize onto the existing references list"
    );
    let dependencies =
        load_reference_manifests(&manifest).expect("load dependency manifests from app manifest");
    let mut package_names = dependencies
        .into_iter()
        .map(|dependency| {
            dependency
                .package
                .expect("dependency manifest should define a package")
                .name
        })
        .collect::<Vec<_>>();
    package_names.sort();

    assert_eq!(package_names, vec!["dep", "extra", "legacy"]);
}

#[test]
fn load_project_manifest_rejects_non_local_dependency_entries() {
    let temp = TempDir::new("ql-project-manifest-dependencies-invalid");
    let app_root = temp.path().join("app");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = { version = "0.1.0" }
"#,
    );

    let error =
        load_project_manifest(&app_root).expect_err("invalid dependency syntax should fail");
    let ProjectError::Parse { message, .. } = error else {
        panic!("invalid dependency syntax should surface a manifest parse error");
    };
    assert!(
        message.contains("currently only supports local path dependencies"),
        "expected unsupported dependency form error, got: {message}"
    );
}

#[test]
fn load_project_manifest_accepts_package_default_profile() {
    let temp = TempDir::new("ql-project-manifest-profile");
    let app_root = temp.path().join("app");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );

    let manifest = load_project_manifest(&app_root).expect("load app manifest");
    assert_eq!(
        manifest
            .profile
            .as_ref()
            .map(|profile| profile.default.as_str()),
        Some("release"),
        "package manifests should preserve the declared default build profile"
    );
}

#[test]
fn load_project_manifest_accepts_workspace_default_profile() {
    let temp = TempDir::new("ql-project-manifest-profile-workspace");
    let workspace_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );

    let manifest = load_project_manifest(&workspace_root).expect("load workspace manifest");
    assert!(
        manifest
            .profile
            .as_ref()
            .is_some_and(|profile| profile.default.as_str() == "release"),
        "workspace manifests should preserve the declared default build profile"
    );
}

#[test]
fn discover_workspace_build_targets_inherits_workspace_default_profile() {
    let temp = TempDir::new("ql-project-workspace-profile-inherit");
    let workspace_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );

    let manifest = load_project_manifest(&workspace_root).expect("load workspace manifest");
    let members = discover_workspace_build_targets(&manifest).expect("discover workspace targets");
    assert_eq!(members.len(), 1, "expected one workspace member");
    assert_eq!(
        members[0].default_profile.map(|profile| profile.as_str()),
        Some("release"),
        "workspace default profile should flow into members that do not override it"
    );
}

#[test]
fn discover_workspace_build_targets_prefers_member_profile_over_workspace_default() {
    let temp = TempDir::new("ql-project-workspace-profile-override");
    let workspace_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );

    let manifest = load_project_manifest(&workspace_root).expect("load workspace manifest");
    let members = discover_workspace_build_targets(&manifest).expect("discover workspace targets");
    assert_eq!(members.len(), 1, "expected one workspace member");
    assert_eq!(
        members[0].default_profile.map(|profile| profile.as_str()),
        Some("debug"),
        "package profile should override the workspace default profile"
    );
}
