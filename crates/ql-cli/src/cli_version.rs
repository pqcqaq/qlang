pub(crate) const CLI_NAME: &str = "ql";
pub(crate) const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn is_version_command(command: &str) -> bool {
    matches!(command, "--version" | "-V" | "version")
}

pub(crate) fn version_text(binary_name: &str) -> String {
    format!("{binary_name} {CLI_VERSION}")
}

#[cfg(test)]
mod tests {
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
}
