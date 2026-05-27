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

fn render_dependency_bridge_prep_error(command_label: &str, error: impl Display) -> [String; 1] {
    [format!("error: {command_label} {error}")]
}

pub(crate) fn dependency_interface_load_message(
    interface_path: &Path,
    error: impl Display,
) -> String {
    format!(
        "failed to load referenced package interface `{}`: {error}",
        normalize_path(interface_path)
    )
}

pub(crate) fn dependency_source_read_message(source_path: &Path, error: impl Display) -> String {
    format!(
        "failed to access dependency source `{}`: {error}",
        normalize_path(source_path)
    )
}

pub(crate) fn dependency_source_parse_message(source_path: &Path, bridge_context: &str) -> String {
    format!(
        "failed to parse dependency source `{}` while preparing {bridge_context}",
        normalize_path(source_path)
    )
}

fn render_package_under_test_source_read_failure(
    command_label: &str,
    source_path: &Path,
    error: impl Display,
) -> [String; 1] {
    [format!(
        "error: {command_label} failed to access package-under-test source `{}`: {error}",
        normalize_path(source_path)
    )]
}

fn render_package_under_test_source_parse_failure(
    command_label: &str,
    source_path: &Path,
) -> [String; 1] {
    [format!(
        "error: {command_label} failed to parse package-under-test source `{}` while preparing test bridges",
        normalize_path(source_path)
    )]
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
            "error: {command_label} {}",
            dependency_interface_load_message(interface_path, error)
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
            "error: {command_label} {}",
            dependency_source_read_message(source_path, error)
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
            "error: {command_label} {}",
            dependency_source_parse_message(source_path, bridge_context)
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

pub(crate) fn report_dependency_bridge_prep_error(command_label: &str, error: impl Display) {
    for line in render_dependency_bridge_prep_error(command_label, error) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_package_under_test_source_read_failure(
    command_label: &str,
    source_path: &Path,
    error: impl Display,
) {
    for line in render_package_under_test_source_read_failure(command_label, source_path, error) {
        eprintln!("{line}");
    }
}

pub(crate) fn report_package_under_test_source_parse_failure(
    command_label: &str,
    source_path: &Path,
) {
    for line in render_package_under_test_source_parse_failure(command_label, source_path) {
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
#[path = "dependency_bridge_reporting_tests.rs"]
mod tests;
