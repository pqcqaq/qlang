use super::*;

#[test]
fn append_workspace_manifest_member_adds_member_and_trailing_newline() {
    let rendered = append_workspace_manifest_member(
        "[workspace]\nmembers = [\"packages/core\"]\n",
        "packages/app",
    )
    .expect("append workspace member");

    assert!(rendered.contains("\"packages/core\""));
    assert!(rendered.contains("\"packages/app\""));
    assert!(rendered.ends_with('\n'));
}

#[test]
fn remove_workspace_manifest_member_removes_exact_member() {
    let rendered = remove_workspace_manifest_member(
        "[workspace]\nmembers = [\"packages/core\", \"packages/app\"]\n",
        "packages/core",
    )
    .expect("remove workspace member");

    assert!(!rendered.contains("\"packages/core\""));
    assert!(rendered.contains("\"packages/app\""));
}

#[test]
fn remove_workspace_manifest_member_reports_missing_member() {
    let error = remove_workspace_manifest_member(
        "[workspace]\nmembers = [\"packages/app\"]\n",
        "packages/core",
    )
    .expect_err("missing member should fail");

    assert_eq!(
        error,
        "workspace manifest does not declare member `packages/core`"
    );
}
