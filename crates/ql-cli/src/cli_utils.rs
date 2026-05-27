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
#[path = "cli_utils_tests.rs"]
mod tests;
