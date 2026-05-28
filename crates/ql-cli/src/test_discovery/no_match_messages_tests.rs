use std::path::Path;

use super::no_match_messages::{
    test_no_matching_filter_message, test_no_matching_target_message, test_no_tests_message,
};

#[test]
fn no_tests_message_includes_package_selector_context() {
    let message = test_no_tests_message(Path::new("workspace"), Some("core"));

    assert_eq!(
        message,
        "`ql test` found no `.ql` test files for package `core` under `workspace`"
    );
}

#[test]
fn no_tests_message_without_package_selector_names_request_path() {
    let message = test_no_tests_message(Path::new("workspace"), None);

    assert_eq!(
        message,
        "`ql test` found no `.ql` test files under `workspace`"
    );
}

#[test]
fn no_matching_filter_message_includes_package_selector_context() {
    let message = test_no_matching_filter_message(Path::new("workspace"), "slow", Some("core"));

    assert_eq!(
        message,
        "`ql test` found no test files matching `slow` for package `core` under `workspace`"
    );
}

#[test]
fn no_matching_filter_message_without_package_selector_names_request_path() {
    let message = test_no_matching_filter_message(Path::new("workspace"), "slow", None);

    assert_eq!(
        message,
        "`ql test` found no test files matching `slow` under `workspace`"
    );
}

#[test]
fn no_matching_target_message_includes_package_selector_context() {
    let message =
        test_no_matching_target_message(Path::new("workspace"), "tests/smoke.ql", Some("core"));

    assert_eq!(
        message,
        "`ql test` found no test target `tests/smoke.ql` for package `core` under `workspace`"
    );
}

#[test]
fn no_matching_target_message_without_package_selector_names_request_path() {
    let message = test_no_matching_target_message(Path::new("workspace"), "tests/smoke.ql", None);

    assert_eq!(
        message,
        "`ql test` found no test target `tests/smoke.ql` under `workspace`"
    );
}
