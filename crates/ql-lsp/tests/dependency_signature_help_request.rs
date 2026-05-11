mod common;

use common::request::{
    TempDir, did_change_via_request, did_open_via_request, initialize_service_with_workspace_roots,
    inlay_hint_via_request, nth_offset, offset_to_position, signature_help_via_request,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, ParameterLabel, Range, SignatureHelp, Url,
};

#[tokio::test(flavor = "current_thread")]
async fn signature_help_request_uses_dependency_function_and_method_signatures() {
    let temp = TempDir::new("ql-lsp-dependency-signature-help-request");
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

impl Child {
    pub fn get(self, delta: Int, scale: Int) -> Int
}

pub fn exported(left: Int, right: Int) -> Int
pub fn load() -> Child
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let app_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

use demo.dep.exported as run
use demo.dep.load

pub fn main() -> Int {
    let child = load()
    let first = run(1, 2)
    return child.get(3, 4) + first
}
"#,
    );
    let source = std::fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let workspace_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let function_signature = signature_help_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&source, nth_offset(&source, "run(1, ", 1) + "run(1, ".len()),
    )
    .await
    .expect("dependency function signatureHelp should return a signature");
    assert_eq!(function_signature.active_parameter, Some(1));
    assert_eq!(
        function_signature.signatures[0].label,
        "fn exported(left: Int, right: Int) -> Int"
    );
    assert_eq!(
        parameter_labels(&function_signature),
        vec!["left: Int", "right: Int"]
    );

    let method_signature = signature_help_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&source, nth_offset(&source, "get(3, ", 1) + "get(3, ".len()),
    )
    .await
    .expect("dependency method signatureHelp should return a signature");
    assert_eq!(method_signature.active_parameter, Some(1));
    assert_eq!(
        method_signature.signatures[0].label,
        "fn get(self, delta: Int, scale: Int) -> Int"
    );
    assert_eq!(
        parameter_labels(&method_signature),
        vec!["delta: Int", "scale: Int"]
    );

    let full_range = Range::new(
        offset_to_position(&source, 0),
        offset_to_position(&source, source.len()),
    );
    let hints = inlay_hint_via_request(&mut service, app_uri.clone(), full_range)
        .await
        .expect("dependency call inlayHint should return parameter hints");
    assert_parameter_hint(&hints, "left:");
    assert_parameter_hint(&hints, "right:");
    assert_parameter_hint(&hints, "delta:");
    assert_parameter_hint(&hints, "scale:");

    let broken_source = source.replace(
        "    return child.get(3, 4) + first",
        "    return child.get(3, 4) + first\n    let broken =",
    );
    did_change_via_request(&mut service, app_uri.clone(), 2, broken_source.clone()).await;
    let broken_signature = signature_help_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            &broken_source,
            nth_offset(&broken_source, "get(3, ", 1) + "get(3, ".len(),
        ),
    )
    .await
    .expect("dependency method signatureHelp should survive unsaved parse errors");
    assert_eq!(
        parameter_labels(&broken_signature),
        vec!["delta: Int", "scale: Int"]
    );
    let broken_range = Range::new(
        offset_to_position(&broken_source, 0),
        offset_to_position(&broken_source, broken_source.len()),
    );
    let broken_hints = inlay_hint_via_request(&mut service, app_uri, broken_range)
        .await
        .expect("dependency inlayHint should survive unsaved parse errors");
    assert_parameter_hint(&broken_hints, "delta:");
    assert_parameter_hint(&broken_hints, "scale:");
}

fn parameter_labels(signature: &SignatureHelp) -> Vec<String> {
    signature.signatures[0]
        .parameters
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|parameter| match &parameter.label {
            ParameterLabel::Simple(label) => label.clone(),
            ParameterLabel::LabelOffsets(_) => panic!("parameter labels should be strings"),
        })
        .collect()
}

fn assert_parameter_hint(hints: &[InlayHint], expected: &str) {
    assert!(
        hints.iter().any(
            |hint| matches!((&hint.kind, &hint.label), (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == expected)
        ),
        "inlay hints should include dependency parameter `{expected}`: {hints:#?}",
    );
}
