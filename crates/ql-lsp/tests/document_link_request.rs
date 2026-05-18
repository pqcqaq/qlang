mod common;

use common::request::{
    TempDir, did_open_via_request, document_link_via_request,
    initialize_service_with_workspace_roots, nth_offset, offset_to_position,
};
use common::stdlib_real::{real_stdlib_interface_path, write_real_stdlib_workspace};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{Range, Url};

#[tokio::test(flavor = "current_thread")]
async fn document_link_request_links_dependency_imports_to_interfaces() {
    let temp = TempDir::new("ql-lsp-document-link-real-stdlib");
    let app_source = r#"
package demo.app

use std.option.some as option_some
use std.core.clamp_int as clamp_int

pub fn main() -> Int {
    return clamp_int(42, 0, 100)
}
"#;
    let workspace = write_real_stdlib_workspace(&temp, app_source);
    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(
        &mut service,
        workspace.app_uri.clone(),
        app_source.to_owned(),
    )
    .await;

    let links = document_link_via_request(&mut service, workspace.app_uri)
        .await
        .expect("documentLink request should return dependency links");
    assert_import_link(
        &links,
        app_source,
        "std.option.some",
        &real_stdlib_interface_path(&workspace.stdlib_root, "option"),
    );
    assert_import_link(
        &links,
        app_source,
        "std.core.clamp_int",
        &real_stdlib_interface_path(&workspace.stdlib_root, "core"),
    );
}

fn assert_import_link(
    links: &[tower_lsp::lsp_types::DocumentLink],
    source: &str,
    import_path: &str,
    target_path: &std::path::Path,
) {
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
        "documentLink should link `{import_path}` to `{}`: {links:#?}",
        target_path.display()
    );
}
