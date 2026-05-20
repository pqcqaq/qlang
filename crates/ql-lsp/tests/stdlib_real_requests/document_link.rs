use std::path::Path;

use crate::common::request::{TempDir, document_link_via_request, nth_offset, offset_to_position};
use crate::common::stdlib_real::real_stdlib_interface_path;
use crate::support::open_real_stdlib_workspace_with_open_source;
use tower_lsp::lsp_types::{DocumentLink, Range};

#[tokio::test(flavor = "current_thread")]
async fn document_link_request_uses_open_real_stdlib_app_imports() {
    let temp = TempDir::new("ql-lsp-real-stdlib-document-link-open-app");
    let disk_app_source = r#"
package demo.app

pub fn main() -> Int {
    return 0
}
"#;
    let open_app_source = r#"
package demo.app

use std.option.some as option_some
use std.core.clamp_int as clamp_int

pub fn main() -> Int {
    return clamp_int(option_some(42).unwrap_or(0), 0, 100)
}
"#;
    let (mut service, app_uri, stdlib_root) =
        open_real_stdlib_workspace_with_open_source(&temp, disk_app_source, open_app_source).await;

    let links = document_link_via_request(&mut service, app_uri)
        .await
        .expect("real stdlib documentLink should return dependency links from the open app buffer");
    assert_import_link(
        &links,
        open_app_source,
        "std.option.some",
        &real_stdlib_interface_path(&stdlib_root, "option"),
    );
    assert_import_link(
        &links,
        open_app_source,
        "std.core.clamp_int",
        &real_stdlib_interface_path(&stdlib_root, "core"),
    );
}

fn assert_import_link(links: &[DocumentLink], source: &str, import_path: &str, target_path: &Path) {
    let start = nth_offset(source, import_path, 1);
    let expected_range = Range::new(
        offset_to_position(source, start),
        offset_to_position(source, start + import_path.len()),
    );
    let target_path = target_path
        .canonicalize()
        .expect("target interface path should canonicalize");
    assert!(
        links.iter().any(|link| {
            link.range == expected_range
                && link
                    .target
                    .as_ref()
                    .and_then(|uri| uri.to_file_path().ok())
                    .and_then(|path| path.canonicalize().ok())
                    .is_some_and(|path| path == target_path)
        }),
        "documentLink should link open-buffer import `{import_path}` to `{}`: {links:#?}",
        target_path.display(),
    );
}
