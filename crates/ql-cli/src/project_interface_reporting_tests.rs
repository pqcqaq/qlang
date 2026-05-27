use super::*;

#[test]
fn format_project_emit_interface_command_includes_selector_and_flags() {
    let command = format_project_emit_interface_command(
        Some("pkg/qlang.toml"),
        Some(Path::new("out/pkg.qi")),
        true,
        true,
    );

    assert_eq!(
        command,
        "ql project emit-interface pkg/qlang.toml --changed-only --check --output out/pkg.qi"
    );
}

#[test]
fn format_workspace_member_emit_rerun_command_keeps_check_mode_optional() {
    assert_eq!(
        format_workspace_member_emit_rerun_command("pkg/qlang.toml", false, false),
        "ql project emit-interface pkg/qlang.toml"
    );
    assert_eq!(
        format_workspace_member_emit_rerun_command("pkg/qlang.toml", true, true),
        "ql project emit-interface pkg/qlang.toml --changed-only --check"
    );
}
