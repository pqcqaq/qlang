pub(crate) const CLI_NAME: &str = "ql";
pub(crate) const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn is_version_command(command: &str) -> bool {
    matches!(command, "--version" | "-V" | "version")
}

pub(crate) fn version_text(binary_name: &str) -> String {
    format!("{binary_name} {CLI_VERSION}")
}

#[cfg(test)]
#[path = "cli_version_tests.rs"]
mod tests;
