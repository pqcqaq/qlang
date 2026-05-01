mod common;

use common::request::{
    TempDir, did_open_via_request, initialize_service_with_workspace_roots,
    initialized_service_with_open_documents, nth_offset, offset_to_position,
    prepare_rename_via_request, rename_via_request,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{PrepareRenameResponse, Range, TextEdit, Url};

fn range_for(source: &str, needle: &str, occurrence: usize) -> Range {
    let start = nth_offset(source, needle, occurrence);
    Range::new(
        offset_to_position(source, start),
        offset_to_position(source, start + needle.len()),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn rename_request_returns_same_file_workspace_edits() {
    let temp = TempDir::new("ql-lsp-rename-request-same-file");
    let source_path = temp.write(
        "sample.ql",
        r#"
fn id(value: Int) -> Int {
    return value
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;
    let use_position = offset_to_position(&source, nth_offset(&source, "value", 2));

    let prepare = prepare_rename_via_request(&mut service, uri.clone(), use_position)
        .await
        .expect("prepareRename request should return a rename target");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("prepareRename request should return range plus placeholder")
    };
    assert_eq!(range, range_for(&source, "value", 2));
    assert_eq!(placeholder, "value");

    let edit = rename_via_request(&mut service, uri.clone(), use_position, "input")
        .await
        .expect("rename request should return workspace edits");
    let changes = edit
        .changes
        .expect("rename request should use simple document changes");
    let edits = changes
        .get(&uri)
        .expect("rename request should return edits for the open document");

    assert_eq!(
        edits,
        &vec![
            TextEdit::new(range_for(&source, "value", 1), "input".to_owned()),
            TextEdit::new(range_for(&source, "value", 2), "input".to_owned()),
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn workspace_rename_request_updates_source_root_definition_and_consumers() {
    let temp = TempDir::new("ql-lsp-rename-request-workspace-root");
    let workspace_root = temp.path().join("workspace");
    let app_source = r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    return measure(1)
}
"#;
    let task_source = r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
}
"#;
    let core_source = r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#;
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
    let task_path = temp.write("workspace/packages/app/src/task.ql", task_source);
    let core_path = temp.write("workspace/packages/core/src/lib.ql", core_source);
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;
    did_open_via_request(&mut service, task_uri.clone(), task_source.to_owned()).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.to_owned()).await;
    let import_use_position = offset_to_position(app_source, nth_offset(app_source, "measure", 2));

    let prepare = prepare_rename_via_request(&mut service, app_uri.clone(), import_use_position)
        .await
        .expect("workspace prepareRename request should return import use target");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("workspace prepareRename request should return range plus placeholder")
    };
    assert_eq!(range, range_for(app_source, "measure", 2));
    assert_eq!(placeholder, "measure");

    let edit = rename_via_request(&mut service, app_uri.clone(), import_use_position, "score")
        .await
        .expect("workspace rename request should return edits");
    let changes = edit
        .changes
        .expect("workspace rename request should use simple document changes");

    assert_eq!(
        changes
            .get(&app_uri)
            .expect("workspace rename should edit app source"),
        &vec![
            TextEdit::new(range_for(app_source, "measure", 1), "score".to_owned()),
            TextEdit::new(range_for(app_source, "measure", 2), "score".to_owned()),
        ]
    );
    assert_eq!(
        changes
            .get(&task_uri)
            .expect("workspace rename should edit sibling app source"),
        &vec![
            TextEdit::new(range_for(task_source, "measure", 1), "score".to_owned()),
            TextEdit::new(range_for(task_source, "measure", 2), "score".to_owned()),
        ]
    );
    assert_eq!(
        changes
            .get(&core_uri)
            .expect("workspace rename should edit defining source"),
        &vec![
            TextEdit::new(range_for(core_source, "measure", 1), "score".to_owned()),
            TextEdit::new(range_for(core_source, "measure", 2), "score".to_owned()),
        ]
    );
}
