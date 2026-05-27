use super::*;

fn workspace_manifest() -> ql_project::ProjectManifest {
    ql_project::ProjectManifest {
        manifest_path: PathBuf::from("workspace/qlang.toml"),
        package: None,
        workspace: None,
        references: ql_project::ReferencesManifest::default(),
        profile: None,
        lib: None,
        bins: Vec::new(),
    }
}

#[test]
fn dependencies_text_marks_workspace_and_external_local_dependencies() {
    let dependencies = vec![
        ProjectDependencyMember {
            member: Some("packages/core".to_owned()),
            dependency_path: "../core".to_owned(),
            package_name: "core".to_owned(),
            manifest_path: PathBuf::from("workspace/packages/core/qlang.toml"),
        },
        ProjectDependencyMember {
            member: None,
            dependency_path: "../vendor/log".to_owned(),
            package_name: "log".to_owned(),
            manifest_path: PathBuf::from("vendor/log/qlang.toml"),
        },
    ];

    let rendered = render_project_dependencies(&workspace_manifest(), "app", &dependencies);

    assert!(rendered.contains("  - packages/core (core)\n"));
    assert!(rendered.contains("  - ../vendor/log (log, local)\n"));
}

#[test]
fn dependents_json_renders_stable_schema() {
    let dependents = vec![ProjectDependentMember {
        member: "packages/app".to_owned(),
        package_name: "app".to_owned(),
        manifest_path: PathBuf::from("workspace/packages/app/qlang.toml"),
    }];

    let rendered = render_project_dependents_json(
        Path::new("workspace"),
        &workspace_manifest(),
        "core",
        &dependents,
    );

    assert!(rendered.contains("\"schema\": \"ql.project.dependents.v1\""));
    assert!(rendered.contains("\"package_name\": \"core\""));
    assert!(rendered.contains("\"member\": \"packages/app\""));
}

#[test]
fn dependency_json_marks_dependency_kind() {
    let workspace_dependency = ProjectDependencyMember {
        member: Some("packages/core".to_owned()),
        dependency_path: "../core".to_owned(),
        package_name: "core".to_owned(),
        manifest_path: PathBuf::from("workspace/packages/core/qlang.toml"),
    };
    let local_dependency = ProjectDependencyMember {
        member: None,
        dependency_path: "../vendor/log".to_owned(),
        package_name: "log".to_owned(),
        manifest_path: PathBuf::from("vendor/log/qlang.toml"),
    };

    assert_eq!(
        project_dependency_json(&workspace_dependency)["kind"],
        "workspace"
    );
    assert_eq!(project_dependency_json(&local_dependency)["kind"], "local");
}
