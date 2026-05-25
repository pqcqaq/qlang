use std::env;
use std::path::{Component, Path, PathBuf};

pub(crate) fn package_check_manifest_path_from_project_error(
    error: &ql_project::ProjectError,
) -> Option<&Path> {
    match error {
        ql_project::ProjectError::PackageNotDefined { path }
        | ql_project::ProjectError::Read { path, .. }
        | ql_project::ProjectError::Parse { path, .. } => Some(path.as_path()),
        ql_project::ProjectError::ManifestNotFound { .. }
        | ql_project::ProjectError::PackageSourceRootNotFound { .. } => None,
    }
}

pub(crate) fn package_missing_name_manifest_path_from_project_error(
    error: &ql_project::ProjectError,
) -> Option<&Path> {
    match error {
        ql_project::ProjectError::PackageNotDefined { path } => Some(path.as_path()),
        ql_project::ProjectError::Parse { path, message }
            if message == "`[package].name` must be present" =>
        {
            Some(path.as_path())
        }
        _ => None,
    }
}

pub(crate) fn validate_project_package_name(package_name: &str) -> Result<(), String> {
    if package_name.trim().is_empty() {
        return Err("requires a non-empty package name".to_owned());
    }
    if package_name == "." || package_name == ".." {
        return Err(format!(
            "does not accept reserved package name `{package_name}`"
        ));
    }
    if package_name.contains(['/', '\\']) {
        return Err(format!(
            "does not accept package name `{package_name}` because it contains a path separator"
        ));
    }
    Ok(())
}

pub(crate) fn absolute_user_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

pub(crate) fn relative_path_from(from: &Path, to: &Path) -> String {
    let from = normalize_path(from);
    let to = normalize_path(to);
    let from_parts = from
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    let to_parts = to
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();

    let mut common = 0;
    while common < from_parts.len()
        && common < to_parts.len()
        && from_parts[common] == to_parts[common]
    {
        common += 1;
    }

    if common == 0 && !from_parts.is_empty() && !to_parts.is_empty() && from_parts[0] != to_parts[0]
    {
        return to;
    }

    let mut relative = Vec::new();
    relative.extend(std::iter::repeat_n(
        "..",
        from_parts.len().saturating_sub(common),
    ));
    relative.extend_from_slice(&to_parts[common..]);
    if relative.is_empty() {
        ".".to_owned()
    } else {
        relative.join("/")
    }
}

pub(crate) fn normalize_path(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        normalized.to_string_lossy().replace('\\', "/")
    }
}

pub(crate) fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

pub(crate) fn json_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push('"');
    for ch in value.chars() {
        match ch {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            '\u{08}' => rendered.push_str("\\b"),
            '\u{0C}' => rendered.push_str("\\f"),
            ch if ch.is_control() => {
                use std::fmt::Write as _;
                write!(rendered, "\\u{:04x}", ch as u32).expect("write escaped json control");
            }
            _ => rendered.push(ch),
        }
    }
    rendered.push('"');
    rendered
}

#[cfg(test)]
mod tests {
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
}
