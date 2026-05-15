use std::fs;

use crate::common::request::{
    TempDir, nth_offset, offset_to_position, prepare_rename_via_request, rename_via_request,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::{assert_edit, open_real_stdlib_workspace, range_for, range_for_in_context};
use tower_lsp::lsp_types::{PrepareRenameResponse, Url};

#[tokio::test(flavor = "current_thread")]
async fn rename_request_updates_current_real_stdlib_source_roots() {
    let temp = TempDir::new("ql-lsp-real-stdlib-rename-request");
    let app_source = r#"
package demo.app

use std.core.max_int as maximum

pub fn main() -> Int {
    return maximum(1, 2)
}
"#;
    let (mut service, app_uri, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;
    let core_source_path = real_stdlib_source_path(&stdlib_root, "core");
    let core_source = fs::read_to_string(&core_source_path)
        .expect("temp std.core source should exist")
        .replace("\r\n", "\n");
    let core_uri = Url::from_file_path(&core_source_path)
        .expect("temp std.core source path should convert to URI");
    let import_position = offset_to_position(app_source, nth_offset(app_source, "max_int", 1));

    let prepare = prepare_rename_via_request(&mut service, app_uri.clone(), import_position)
        .await
        .expect("real stdlib prepareRename should target imported function");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("prepareRename should return range plus placeholder")
    };
    assert_eq!(range, range_for(app_source, "max_int", 1));
    assert_eq!(placeholder, "max_int");

    let edit = rename_via_request(
        &mut service,
        app_uri.clone(),
        import_position,
        "largest_int",
    )
    .await
    .expect("real stdlib rename should return workspace edit");
    let changes = edit
        .changes
        .expect("real stdlib rename should use simple workspace changes");

    assert_edit(
        changes
            .get(&app_uri)
            .expect("rename should edit importing app source"),
        range_for(app_source, "max_int", 1),
        "largest_int",
    );
    assert_edit(
        changes
            .get(&core_uri)
            .expect("rename should edit temp std.core source"),
        range_for(&core_source, "max_int", 1),
        "largest_int",
    );
    assert_edit(
        changes
            .get(&core_uri)
            .expect("rename should edit std.core call sites"),
        range_for_in_context(
            &core_source,
            "max_int",
            "max_int(first_bound, second_bound)",
            1,
        ),
        "largest_int",
    );
}
