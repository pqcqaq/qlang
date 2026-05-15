use crate::common::request::{
    TempDir, code_action_resolve_via_request, code_action_via_request, nth_offset,
    offset_to_position,
};
use crate::support::{
    assert_code_action, assert_edit, open_real_stdlib_workspace, unresolved_type_diagnostic,
};
use tower_lsp::lsp_types::{CodeActionOrCommand, Range};

#[tokio::test(flavor = "current_thread")]
async fn code_action_request_auto_imports_current_real_stdlib_types() {
    let temp = TempDir::new("ql-lsp-real-stdlib-code-action-request");
    let app_source = r#"
package demo.app

pub fn main(value: Option[Int]) -> Int {
    return 0
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;
    let diagnostic = unresolved_type_diagnostic(app_source, "Option");

    let actions = code_action_via_request(
        &mut service,
        app_uri.clone(),
        diagnostic.range,
        vec![diagnostic.clone()],
    )
    .await
    .expect("real stdlib codeAction should return auto-import quickfixes");

    let changes = assert_code_action(&actions, "Import `std.option.Option`");
    assert_eq!(
        changes.len(),
        1,
        "real stdlib import should not edit manifest"
    );
    assert_edit(
        changes
            .get(&app_uri)
            .expect("auto-import should edit the importing app source"),
        Range::new(
            offset_to_position(app_source, nth_offset(app_source, "\n\npub fn main", 1) + 1),
            offset_to_position(app_source, nth_offset(app_source, "\n\npub fn main", 1) + 1),
        ),
        "use std.option.Option\n",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn code_action_resolve_preserves_current_real_stdlib_auto_import_quickfix() {
    let temp = TempDir::new("ql-lsp-real-stdlib-code-action-resolve");
    let app_source = r#"
package demo.app

pub fn main(value: Option[Int]) -> Int {
    return 0
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;
    let diagnostic = unresolved_type_diagnostic(app_source, "Option");

    let actions = code_action_via_request(
        &mut service,
        app_uri,
        diagnostic.range,
        vec![diagnostic.clone()],
    )
    .await
    .expect("real stdlib codeAction should return auto-import quickfixes");
    let action = actions
        .iter()
        .find(|action| {
            matches!(
                action,
                CodeActionOrCommand::CodeAction(action)
                    if action.title == "Import `std.option.Option`"
            )
        })
        .unwrap_or_else(|| {
            panic!("real stdlib code actions should include option import: {actions:#?}")
        })
        .clone();
    let CodeActionOrCommand::CodeAction(action) = action else {
        panic!("real stdlib option import should be a code action")
    };

    let resolved = code_action_resolve_via_request(&mut service, action.clone()).await;

    assert_eq!(resolved, action);
}
