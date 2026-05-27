use std::path::{Path, PathBuf};

use super::*;

#[test]
fn normalize_path_collapses_current_and_parent_components() {
    assert_eq!(
        normalize_path(Path::new("workspace/./packages/app/../core")),
        "workspace/packages/core"
    );
    assert_eq!(normalize_path(Path::new("")), ".");
}

#[test]
fn normalize_line_endings_converts_crlf_to_lf() {
    assert_eq!(normalize_line_endings("a\r\nb\nc\r\n"), "a\nb\nc\n");
}

#[test]
fn json_string_escapes_json_control_characters() {
    assert_eq!(
        json_string("a\"b\\c\nd\re\t\u{08}f\u{0C}\u{01}"),
        "\"a\\\"b\\\\c\\nd\\re\\t\\bf\\f\\u0001\""
    );
}

#[test]
fn relative_path_from_renders_shared_roots_and_distinct_roots() {
    assert_eq!(
        relative_path_from(
            Path::new("workspace/packages/app"),
            Path::new("workspace/stdlib/core")
        ),
        "../../stdlib/core"
    );
    assert_eq!(
        relative_path_from(Path::new("a/b"), Path::new("x/y")),
        "x/y"
    );
    assert_eq!(relative_path_from(Path::new("a/b"), Path::new("a/b")), ".");
}

#[test]
fn absolute_user_path_keeps_absolute_and_anchors_relative_paths() {
    let current = env::current_dir().expect("current dir");
    assert_eq!(absolute_user_path(&current), current);

    let resolved = absolute_user_path(Path::new("src"));
    assert!(resolved.is_absolute());
    assert!(resolved.ends_with("src"));
}

#[test]
fn validate_project_package_name_rejects_empty_reserved_and_paths() {
    assert!(validate_project_package_name("std.core").is_ok());
    assert!(validate_project_package_name("").is_err());
    assert!(validate_project_package_name("   ").is_err());
    assert!(validate_project_package_name(".").is_err());
    assert!(validate_project_package_name("..").is_err());
    assert!(validate_project_package_name("foo/bar").is_err());
    assert!(validate_project_package_name("foo\\bar").is_err());
}

#[test]
fn project_error_manifest_path_helpers_preserve_existing_contract() {
    let path = PathBuf::from("qlang.toml");
    let missing_name = ql_project::ProjectError::Parse {
        path: path.clone(),
        message: "`[package].name` must be present".to_owned(),
    };
    assert_eq!(
        package_check_manifest_path_from_project_error(&missing_name),
        Some(path.as_path())
    );
    assert_eq!(
        package_missing_name_manifest_path_from_project_error(&missing_name),
        Some(path.as_path())
    );

    let not_found = ql_project::ProjectError::ManifestNotFound {
        start: PathBuf::from("src"),
    };
    assert_eq!(
        package_check_manifest_path_from_project_error(&not_found),
        None
    );
    assert_eq!(
        package_missing_name_manifest_path_from_project_error(&not_found),
        None
    );
}
