use crate::test_reporting::TestTarget;

pub(crate) fn list_test_targets(targets: &[TestTarget]) {
    print!("{}", render_test_target_listing(targets));
}

pub(super) fn render_test_target_listing(targets: &[TestTarget]) -> String {
    let mut rendered = String::new();
    for target in targets {
        rendered.push_str(&target.display_path);
        rendered.push('\n');
    }
    rendered.push('\n');
    rendered.push_str(&format!("test listing: {} discovered\n", targets.len()));
    rendered
}
