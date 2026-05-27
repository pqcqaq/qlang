use std::path::Path;

use super::*;

#[test]
fn dependency_bridge_prep_error_preserves_single_line_contract() {
    let lines = render_dependency_bridge_prep_error("`ql build`", "invalid manifest");

    assert_eq!(lines, ["error: `ql build` invalid manifest"]);
}

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
fn package_under_test_source_failure_lines_preserve_read_and_parse_contracts() {
    let read_lines = render_package_under_test_source_read_failure(
        "`ql test`",
        Path::new("workspace/app/src/lib.ql"),
        "access denied",
    );
    let parse_lines = render_package_under_test_source_parse_failure(
        "`ql test`",
        Path::new("workspace/app/src/lib.ql"),
    );

    assert_eq!(
        read_lines,
        [
            "error: `ql test` failed to access package-under-test source `workspace/app/src/lib.ql`: access denied",
        ]
    );
    assert_eq!(
        parse_lines,
        [
            "error: `ql test` failed to parse package-under-test source `workspace/app/src/lib.ql` while preparing test bridges",
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
    let generic_lines = render_package_under_test_unsupported_generic_function("`ql test`", "map");

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
