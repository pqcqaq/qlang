use super::*;

#[test]
fn graph_rerun_command_preserves_normalized_path() {
    assert_eq!(
        format_project_graph_command("packages/app/qlang.toml"),
        "ql project graph packages/app/qlang.toml"
    );
}
