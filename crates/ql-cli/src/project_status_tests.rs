use super::*;

fn member_with_status(status: InterfaceArtifactStatus) -> ProjectStatusMember {
    ProjectStatusMember {
        member: Some("packages/app".to_owned()),
        package_name: "app".to_owned(),
        manifest_path: PathBuf::from("packages/app/qlang.toml"),
        default_profile: None,
        targets: Vec::new(),
        dependencies: Vec::new(),
        interface: ProjectStatusInterface {
            path: PathBuf::from("packages/app/target/qlang/app.qi"),
            status,
            detail: None,
            stale_reasons: Vec::new(),
        },
    }
}

#[test]
fn status_label_prioritizes_invalid_or_unreadable_interfaces() {
    let members = vec![
        member_with_status(InterfaceArtifactStatus::Valid),
        member_with_status(InterfaceArtifactStatus::Invalid),
    ];

    assert_eq!(project_status_label(&members), "needs-attention");
}

#[test]
fn status_label_reports_interface_sync_for_missing_or_stale_interfaces() {
    let members = vec![member_with_status(InterfaceArtifactStatus::Missing)];

    assert_eq!(project_status_label(&members), "needs-interface-sync");
}

#[test]
fn member_label_includes_workspace_member_when_present() {
    let member = member_with_status(InterfaceArtifactStatus::Valid);

    assert_eq!(project_status_member_label(&member), "packages/app (app)");
}
