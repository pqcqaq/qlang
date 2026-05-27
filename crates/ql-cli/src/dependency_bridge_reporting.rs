use std::fmt::Display;
use std::path::Path;

use crate::cli_utils::normalize_path;

fn dependency_bridge_owner_note(owner_manifest_path: &Path, bridge_context: &str) -> String {
    format!(
        "note: while preparing {bridge_context} for `{}`",
        normalize_path(owner_manifest_path)
    )
}

fn dependency_bridge_package_note(dependency_package: &str) -> String {
    format!("note: dependency package: `{dependency_package}`")
}

fn render_dependency_interface_load_failure(
    command_label: &str,
    owner_manifest_path: &Path,
    bridge_context: &str,
    interface_path: &Path,
    error: impl Display,
) -> [String; 2] {
    [
        format!(
            "error: {command_label} failed to load referenced package interface `{}`: {error}",
            normalize_path(interface_path)
        ),
        dependency_bridge_owner_note(owner_manifest_path, bridge_context),
    ]
}

fn render_dependency_source_read_failure(
    command_label: &str,
    owner_manifest_path: &Path,
    bridge_context: &str,
    source_path: &Path,
    error: impl Display,
) -> [String; 2] {
    [
        format!(
            "error: {command_label} failed to access dependency source `{}`: {error}",
            normalize_path(source_path)
        ),
        dependency_bridge_owner_note(owner_manifest_path, bridge_context),
    ]
}

fn render_dependency_source_parse_failure(
    command_label: &str,
    dependency_package: &str,
    source_path: &Path,
    bridge_context: &str,
) -> [String; 2] {
    [
        format!(
            "error: {command_label} failed to parse dependency source `{}` while preparing {bridge_context}",
            normalize_path(source_path)
        ),
        dependency_bridge_package_note(dependency_package),
    ]
}

pub(crate) fn report_dependency_interface_load_failure(
    command_label: &str,
    owner_manifest_path: &Path,
    bridge_context: &str,
    interface_path: &Path,
    error: impl Display,
) {
    for line in render_dependency_interface_load_failure(
        command_label,
        owner_manifest_path,
        bridge_context,
        interface_path,
        error,
    ) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_dependency_source_read_failure(
    command_label: &str,
    owner_manifest_path: &Path,
    bridge_context: &str,
    source_path: &Path,
    error: impl Display,
) {
    for line in render_dependency_source_read_failure(
        command_label,
        owner_manifest_path,
        bridge_context,
        source_path,
        error,
    ) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_dependency_source_parse_failure(
    command_label: &str,
    dependency_package: &str,
    source_path: &Path,
    bridge_context: &str,
) {
    for line in render_dependency_source_parse_failure(
        command_label,
        dependency_package,
        source_path,
        bridge_context,
    ) {
        eprintln!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_load_failure_lines_preserve_context() {
        let lines = render_dependency_interface_load_failure(
            "`ql build`",
            Path::new("workspace/app/qlang.toml"),
            "dependency public function wrappers",
            Path::new("workspace/dep/dep.qi"),
            "invalid header",
        );

        assert_eq!(
            lines,
            [
                "error: `ql build` failed to load referenced package interface `workspace/dep/dep.qi`: invalid header",
                "note: while preparing dependency public function wrappers for `workspace/app/qlang.toml`",
            ]
        );
    }

    #[test]
    fn source_failure_lines_preserve_read_and_parse_contracts() {
        let read_lines = render_dependency_source_read_failure(
            "`ql test`",
            Path::new("workspace/app/qlang.toml"),
            "dependency public type bridges",
            Path::new("workspace/dep/src/lib.ql"),
            "access denied",
        );
        let parse_lines = render_dependency_source_parse_failure(
            "`ql test`",
            "dep",
            Path::new("workspace/dep/src/lib.ql"),
            "public type bridges",
        );

        assert_eq!(
            read_lines,
            [
                "error: `ql test` failed to access dependency source `workspace/dep/src/lib.ql`: access denied",
                "note: while preparing dependency public type bridges for `workspace/app/qlang.toml`",
            ]
        );
        assert_eq!(
            parse_lines,
            [
                "error: `ql test` failed to parse dependency source `workspace/dep/src/lib.ql` while preparing public type bridges",
                "note: dependency package: `dep`",
            ]
        );
    }
}
