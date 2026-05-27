use crate::cli_version::{is_version_command, version_text};

#[test]
fn version_text_includes_workspace_package_version() {
    assert_eq!(
        version_text("ql"),
        format!("ql {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn version_command_recognizes_global_aliases() {
    for command in ["--version", "-V", "version"] {
        assert!(
            is_version_command(command),
            "expected {command} to be recognized"
        );
    }
    assert!(!is_version_command("check"));
}
