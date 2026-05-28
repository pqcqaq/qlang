use std::path::Path;

use super::package_selector_messages::package_selector_mismatch_message;

#[test]
fn package_selector_mismatch_message_names_request_root() {
    let message = package_selector_mismatch_message(Path::new("workspace"));

    assert_eq!(
        message,
        "package selector matched no workspace members under `workspace`"
    );
}

#[test]
fn package_selector_mismatch_message_normalizes_nested_request_root() {
    let message = package_selector_mismatch_message(Path::new("workspace/packages/app"));

    assert_eq!(
        message,
        "package selector matched no workspace members under `workspace/packages/app`"
    );
}
