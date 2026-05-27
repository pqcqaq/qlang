use super::*;

#[test]
fn render_package_manifest_quotes_dotted_dependency_keys() {
    let manifest = render_package_manifest(
        "demo-app",
        &[("std.core".to_owned(), "../stdlib/packages/core".to_owned())],
    );

    assert_eq!(
        manifest,
        "[package]\nname = \"demo-app\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n"
    );
}

#[test]
fn render_package_manifest_escapes_string_literals() {
    let manifest = render_package_manifest(
        "demo\"app",
        &[("local_dep".to_owned(), "..\\deps\tcore\nnext".to_owned())],
    );

    assert_eq!(
        manifest,
        "[package]\nname = \"demo\\\"app\"\n\n[dependencies]\nlocal_dep = \"..\\\\deps\\tcore\\nnext\"\n"
    );
}

#[test]
fn render_workspace_manifest_uses_conventional_packages_member() {
    assert_eq!(
        render_workspace_manifest("app"),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
}
