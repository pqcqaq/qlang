mod common;

use common::request::{
    TempDir, code_action_via_request, did_open_via_request,
    initialize_service_with_workspace_roots, nth_offset, offset_to_position,
};
use ql_diagnostics::UNRESOLVED_TYPE_CODE;
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{
    CodeActionOrCommand, Diagnostic, NumberOrString, Position, Range, TextEdit, Url,
};

fn unresolved_type_diagnostic(source: &str, name: &str) -> Diagnostic {
    let start = nth_offset(source, name, 1);
    Diagnostic {
        range: Range::new(
            offset_to_position(source, start),
            offset_to_position(source, start + name.len()),
        ),
        severity: None,
        code: Some(NumberOrString::String(UNRESOLVED_TYPE_CODE.to_owned())),
        code_description: None,
        source: None,
        message: format!("unresolved type `{name}`"),
        related_information: None,
        tags: None,
        data: None,
    }
}

fn setup_workspace(temp: &TempDir, app_manifest: &str, app_source: &str) -> (Url, Url, Url) {
    let workspace_root = temp.path().join("workspace");
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
    let app_manifest_path = temp.write("workspace/packages/app/qlang.toml", app_manifest);
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
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
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {}
"#,
    );

    (
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI"),
        Url::from_file_path(&app_path).expect("app source path should convert to URI"),
        Url::from_file_path(&app_manifest_path).expect("app manifest path should convert to URI"),
    )
}

fn action_changes(action: &CodeActionOrCommand) -> &std::collections::HashMap<Url, Vec<TextEdit>> {
    let CodeActionOrCommand::CodeAction(action) = action else {
        panic!("expected code action, got {action:#?}")
    };
    action
        .edit
        .as_ref()
        .expect("code action should contain workspace edit")
        .changes
        .as_ref()
        .expect("workspace edit should contain direct changes")
}

#[tokio::test(flavor = "current_thread")]
async fn code_action_request_auto_imports_unresolved_type_from_existing_dependency() {
    let temp = TempDir::new("ql-lsp-code-action-type-import");
    let app_source = r#"package demo.app

pub fn build(config: Config) -> Int {
    return 1
}
"#;
    let app_manifest = r#"
[package]
name = "app"

[dependencies]
core = { path = "../core" }
"#;
    let (workspace_root_uri, app_uri, _) = setup_workspace(&temp, app_manifest, app_source);
    let diagnostic = unresolved_type_diagnostic(app_source, "Config");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let actions = code_action_via_request(
        &mut service,
        app_uri.clone(),
        diagnostic.range,
        vec![diagnostic.clone()],
    )
    .await
    .expect("code action request should return actions");

    assert_eq!(actions.len(), 1, "actual actions: {actions:#?}");
    let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
        panic!("expected code action, got {:#?}", actions[0])
    };
    assert_eq!(action.title, "Import `demo.core.Config`");
    assert_eq!(action.diagnostics, Some(vec![diagnostic]));
    let changes = action_changes(&actions[0]);
    assert_eq!(changes.len(), 1, "actual changes: {changes:#?}");
    assert_eq!(
        changes.get(&app_uri),
        Some(&vec![TextEdit::new(
            Range::new(Position::new(1, 0), Position::new(1, 0)),
            "use demo.core.Config\n".to_owned(),
        )]),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn code_action_request_auto_imports_type_and_adds_missing_workspace_dependency() {
    let temp = TempDir::new("ql-lsp-code-action-type-import-add-dependency");
    let app_source = r#"package demo.app

pub fn build(config: Config) -> Int {
    return 1
}
"#;
    let app_manifest = r#"
[package]
name = "app"
"#;
    let (workspace_root_uri, app_uri, app_manifest_uri) =
        setup_workspace(&temp, app_manifest, app_source);
    let diagnostic = unresolved_type_diagnostic(app_source, "Config");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let actions = code_action_via_request(
        &mut service,
        app_uri.clone(),
        diagnostic.range,
        vec![diagnostic.clone()],
    )
    .await
    .expect("code action request should return actions");

    assert_eq!(actions.len(), 1, "actual actions: {actions:#?}");
    let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
        panic!("expected code action, got {:#?}", actions[0])
    };
    assert_eq!(
        action.title,
        "Import `demo.core.Config` and add dependency `core`"
    );
    assert_eq!(action.diagnostics, Some(vec![diagnostic]));
    let changes = action_changes(&actions[0]);
    assert_eq!(changes.len(), 2, "actual changes: {changes:#?}");
    assert_eq!(
        changes.get(&app_uri),
        Some(&vec![TextEdit::new(
            Range::new(Position::new(1, 0), Position::new(1, 0)),
            "use demo.core.Config\n".to_owned(),
        )]),
    );
    let manifest_edits = changes
        .get(&app_manifest_uri)
        .expect("workspace edit should update the app manifest");
    assert_eq!(
        manifest_edits.len(),
        1,
        "actual manifest edits: {manifest_edits:#?}",
    );
    assert!(
        manifest_edits[0]
            .new_text
            .contains("[dependencies]\ncore = \"../core\"\n"),
        "actual manifest edit: {:#?}",
        manifest_edits[0],
    );
}
