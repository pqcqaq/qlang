use super::*;
use std::fs;
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
fn ql_source_file_detection_requires_existing_ql_file() {
    let test_root =
        std::env::temp_dir().join(format!("ql-source-file-detection-{}", std::process::id()));
    let _ = fs::remove_dir_all(&test_root);
    fs::create_dir_all(&test_root).expect("create temp test root");
    let ql_file = test_root.join("main.QL");
    let text_file = test_root.join("main.txt");
    fs::write(&ql_file, "fn main() -> Int { return 0 }\n").expect("write ql file");
    fs::write(&text_file, "not qlang").expect("write text file");

    assert!(is_ql_source_file(&ql_file));
    assert!(!is_ql_source_file(&text_file));
    assert!(!is_ql_source_file(&test_root.join("missing.ql")));

    let _ = fs::remove_dir_all(test_root);
}

#[test]
fn target_selector_parser_records_package_and_target_options() {
    let remaining = vec![
        "--package".to_owned(),
        "app".to_owned(),
        "--target".to_owned(),
        "src/bin/main.ql".to_owned(),
    ];
    let mut selector = ProjectTargetSelector::default();
    let mut index = 0;

    assert_eq!(
        parse_project_target_selector_option("`ql build`", &remaining, &mut index, &mut selector),
        Ok(true)
    );
    assert_eq!(index, 1);
    index += 1;
    assert_eq!(
        parse_project_target_selector_option("`ql build`", &remaining, &mut index, &mut selector),
        Ok(true)
    );

    assert_eq!(selector.package_name, Some("app".to_owned()));
    assert_eq!(
        selector.target,
        Some(ProjectTargetSelectorKind::DisplayPath(
            "src/bin/main.ql".to_owned()
        ))
    );
}

#[test]
fn target_selector_parser_rejects_conflicting_target_selectors() {
    let remaining = vec!["--lib".to_owned(), "--bin".to_owned(), "app".to_owned()];
    let mut selector = ProjectTargetSelector::default();
    let mut index = 0;

    assert_eq!(
        parse_project_target_selector_option("`ql run`", &remaining, &mut index, &mut selector),
        Ok(true)
    );
    index += 1;
    assert_eq!(
        parse_project_target_selector_option("`ql run`", &remaining, &mut index, &mut selector),
        Err(1)
    );
    assert_eq!(selector.target, Some(ProjectTargetSelectorKind::Library));
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

    let resolved = resolve_project_command_path(Path::new("packages/app/qlang.toml"), &selector);

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
