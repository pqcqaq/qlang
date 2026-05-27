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

fn direct_dependency_conflicting_package_note(dependency_package: &str) -> String {
    format!("note: conflicting direct dependency package: `{dependency_package}`")
}

fn direct_dependency_package_note(dependency_package: &str) -> String {
    format!("note: direct dependency package: `{dependency_package}`")
}

fn package_under_test_note(package_name: &str) -> String {
    format!("note: package under test: `{package_name}`")
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

fn render_direct_dependency_symbol_conflict(
    command_label: &str,
    symbol_kind: &str,
    symbol: &str,
    first_package: &str,
    conflicting_package: &str,
    hint: &str,
) -> [String; 4] {
    [
        format!(
            "error: {command_label} found conflicting direct dependency {symbol_kind} imports for `{symbol}`"
        ),
        format!("note: first package: `{first_package}`"),
        format!("note: conflicting package: `{conflicting_package}`"),
        format!("hint: {hint}"),
    ]
}

fn render_direct_dependency_local_conflict(
    command_label: &str,
    bridge_kind: &str,
    symbol: &str,
    dependency_package: &str,
    hint: &str,
) -> [String; 3] {
    [
        format!(
            "error: {command_label} cannot synthesize direct dependency {bridge_kind} bridge for `{symbol}` because the root source already defines the same top-level name"
        ),
        direct_dependency_conflicting_package_note(dependency_package),
        format!("hint: {hint}"),
    ]
}

fn render_direct_dependency_unsupported_generic_function(
    command_label: &str,
    symbol: &str,
    dependency_package: &str,
) -> [String; 3] {
    [
        format!(
            "error: {command_label} cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
        ),
        direct_dependency_package_note(dependency_package),
        "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types".to_owned(),
    ]
}

fn render_package_under_test_symbol_conflict(
    command_label: &str,
    symbol_kind: &str,
    symbol: &str,
    first_package: &str,
    package_name: &str,
) -> [String; 3] {
    [
        format!(
            "error: {command_label} found conflicting package-under-test {symbol_kind} imports for `{symbol}`"
        ),
        format!("note: first package: `{first_package}`"),
        package_under_test_note(package_name),
    ]
}

fn render_package_under_test_local_conflict(
    command_label: &str,
    bridge_kind: &str,
    symbol: &str,
    hint: &str,
) -> [String; 2] {
    [
        format!(
            "error: {command_label} cannot synthesize package-under-test {bridge_kind} bridge for `{symbol}` because the test source already defines the same top-level name"
        ),
        format!("hint: {hint}"),
    ]
}

fn render_package_under_test_unsupported_generic_function(
    command_label: &str,
    symbol: &str,
) -> [String; 2] {
    [
        format!(
            "error: {command_label} cannot synthesize package-under-test public function bridge for generic function `{symbol}` yet"
        ),
        "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types".to_owned(),
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

pub(crate) fn report_direct_dependency_symbol_conflict(
    command_label: &str,
    symbol_kind: &str,
    symbol: &str,
    first_package: &str,
    conflicting_package: &str,
    hint: &str,
) {
    for line in render_direct_dependency_symbol_conflict(
        command_label,
        symbol_kind,
        symbol,
        first_package,
        conflicting_package,
        hint,
    ) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_direct_dependency_local_conflict(
    command_label: &str,
    bridge_kind: &str,
    symbol: &str,
    dependency_package: &str,
    hint: &str,
) {
    for line in render_direct_dependency_local_conflict(
        command_label,
        bridge_kind,
        symbol,
        dependency_package,
        hint,
    ) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_direct_dependency_unsupported_generic_function(
    command_label: &str,
    symbol: &str,
    dependency_package: &str,
) {
    for line in render_direct_dependency_unsupported_generic_function(
        command_label,
        symbol,
        dependency_package,
    ) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_package_under_test_symbol_conflict(
    command_label: &str,
    symbol_kind: &str,
    symbol: &str,
    first_package: &str,
    package_name: &str,
) {
    for line in render_package_under_test_symbol_conflict(
        command_label,
        symbol_kind,
        symbol,
        first_package,
        package_name,
    ) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_package_under_test_local_conflict(
    command_label: &str,
    bridge_kind: &str,
    symbol: &str,
    hint: &str,
) {
    for line in render_package_under_test_local_conflict(command_label, bridge_kind, symbol, hint) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_package_under_test_unsupported_generic_function(
    command_label: &str,
    symbol: &str,
) {
    for line in render_package_under_test_unsupported_generic_function(command_label, symbol) {
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

    #[test]
    fn direct_dependency_symbol_conflict_lines_preserve_context() {
        let lines = render_direct_dependency_symbol_conflict(
            "`ql build`",
            "public function",
            "parse",
            "dep_a",
            "dep_b",
            "keep direct dependency public function names unique until package-qualified dependency call lowering lands",
        );

        assert_eq!(
            lines,
            [
                "error: `ql build` found conflicting direct dependency public function imports for `parse`",
                "note: first package: `dep_a`",
                "note: conflicting package: `dep_b`",
                "hint: keep direct dependency public function names unique until package-qualified dependency call lowering lands",
            ]
        );
    }

    #[test]
    fn direct_dependency_local_and_generic_conflict_lines_preserve_context() {
        let local_lines = render_direct_dependency_local_conflict(
            "`ql build`",
            "public type",
            "Box",
            "dep",
            "rename the local top-level item or avoid importing a direct dependency public type with the same original symbol name",
        );
        let generic_lines =
            render_direct_dependency_unsupported_generic_function("`ql build`", "map", "dep");

        assert_eq!(
            local_lines,
            [
                "error: `ql build` cannot synthesize direct dependency public type bridge for `Box` because the root source already defines the same top-level name",
                "note: conflicting direct dependency package: `dep`",
                "hint: rename the local top-level item or avoid importing a direct dependency public type with the same original symbol name",
            ]
        );
        assert_eq!(
            generic_lines,
            [
                "error: `ql build` cannot synthesize direct dependency public function bridge for generic function `map` yet",
                "note: direct dependency package: `dep`",
                "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types",
            ]
        );
    }

    #[test]
    fn package_under_test_symbol_conflict_lines_preserve_context() {
        let lines = render_package_under_test_symbol_conflict(
            "`ql test`",
            "public function",
            "parse",
            "dep",
            "app",
        );

        assert_eq!(
            lines,
            [
                "error: `ql test` found conflicting package-under-test public function imports for `parse`",
                "note: first package: `dep`",
                "note: package under test: `app`",
            ]
        );
    }

    #[test]
    fn package_under_test_local_and_generic_conflict_lines_preserve_context() {
        let local_lines = render_package_under_test_local_conflict(
            "`ql test`",
            "public type",
            "Box",
            "rename the local top-level item or avoid importing a package-under-test public type with the same original symbol name",
        );
        let generic_lines =
            render_package_under_test_unsupported_generic_function("`ql test`", "map");

        assert_eq!(
            local_lines,
            [
                "error: `ql test` cannot synthesize package-under-test public type bridge for `Box` because the test source already defines the same top-level name",
                "hint: rename the local top-level item or avoid importing a package-under-test public type with the same original symbol name",
            ]
        );
        assert_eq!(
            generic_lines,
            [
                "error: `ql test` cannot synthesize package-under-test public function bridge for generic function `map` yet",
                "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types",
            ]
        );
    }
}
