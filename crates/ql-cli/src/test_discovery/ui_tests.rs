use std::path::Path;

use super::ui::is_project_ui_test;

#[test]
fn project_ui_test_detection_requires_tests_ui_prefix() {
    assert!(is_project_ui_test(
        Path::new("pkg"),
        Path::new("pkg/tests/ui/basic.ql")
    ));
    assert!(!is_project_ui_test(
        Path::new("pkg"),
        Path::new("pkg/tests/smoke.ql")
    ));
    assert!(!is_project_ui_test(
        Path::new("pkg"),
        Path::new("other/tests/ui/basic.ql")
    ));
}
