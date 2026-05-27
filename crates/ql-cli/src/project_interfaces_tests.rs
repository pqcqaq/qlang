use super::*;

#[test]
fn interface_artifact_renderer_preserves_package_header_and_sources() {
    let rendered = render_interface_artifact(
        "app",
        &[
            ("src/lib.ql".to_owned(), "pub fn one() -> Int\n".to_owned()),
            ("src/extra.ql".to_owned(), "pub const two: Int\n".to_owned()),
        ],
    );

    assert_eq!(
        rendered,
        "// qlang interface v1\n// package: app\n\n// source: src/lib.ql\npub fn one() -> Int\n\n// source: src/extra.ql\npub const two: Int\n"
    );
}
