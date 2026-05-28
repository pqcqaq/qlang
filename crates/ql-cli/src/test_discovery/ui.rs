use std::path::Path;

pub(super) fn is_project_ui_test(package_root: &Path, source_path: &Path) -> bool {
    let Ok(relative) = source_path.strip_prefix(package_root) else {
        return false;
    };
    let mut components = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(segment) => segment.to_str(),
            _ => None,
        });
    matches!(components.next(), Some("tests")) && matches!(components.next(), Some("ui"))
}
