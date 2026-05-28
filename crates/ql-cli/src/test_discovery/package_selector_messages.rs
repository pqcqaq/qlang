use std::path::Path;

use crate::cli_utils::normalize_path;

pub(super) fn package_selector_mismatch_message(request_path: &Path) -> String {
    format!(
        "package selector matched no workspace members under `{}`",
        normalize_path(request_path)
    )
}
