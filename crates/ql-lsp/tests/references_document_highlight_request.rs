mod common;

use common::request::{
    TempDir, did_open_via_request, document_highlight_via_request,
    initialize_service_with_workspace_roots, initialized_service_with_open_documents, nth_offset,
    offset_to_position, references_via_request,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{Location, Range, Url};

fn range_for(source: &str, needle: &str, occurrence: usize) -> Range {
    let start = nth_offset(source, needle, occurrence);
    Range::new(
        offset_to_position(source, start),
        offset_to_position(source, start + needle.len()),
    )
}

fn assert_has_location(
    locations: &[Location],
    uri: &Url,
    source: &str,
    needle: &str,
    occurrence: usize,
) {
    let expected = range_for(source, needle, occurrence);
    assert!(
        locations
            .iter()
            .any(|location| location.uri == *uri && location.range == expected),
        "expected {uri} {expected:?} in locations: {locations:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn references_request_includes_workspace_dependency_definition_and_consumers() {
    let temp = TempDir::new("ql-lsp-references-request-workspace");
    let workspace_root = temp.path().join("workspace");
    let core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 1
}
"#;
    let app_source = r#"
package demo.app

use demo.core.helper as helper

pub fn first() -> Int {
    return helper()
}

pub fn second() -> Int {
    return helper()
}
"#;
    let core_path = temp.write("workspace/packages/core/src/lib.ql", core_source);
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
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

[dependencies]
core = { path = "../core" }
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.to_owned()).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;

    let references = references_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "helper", 3)),
        true,
    )
    .await
    .expect("references request should return workspace dependency references");

    assert_eq!(
        references.len(),
        4,
        "references request should return only the definition, import alias, and two consumers"
    );
    assert_has_location(&references, &core_uri, core_source, "helper", 1);
    assert_has_location(&references, &app_uri, app_source, "helper", 2);
    assert_has_location(&references, &app_uri, app_source, "helper", 3);
    assert_has_location(&references, &app_uri, app_source, "helper", 4);
}

#[tokio::test(flavor = "current_thread")]
async fn document_highlight_request_keeps_current_file_definition_and_usages() {
    let temp = TempDir::new("ql-lsp-document-highlight-request");
    let source_path = temp.write(
        "sample.ql",
        r#"
fn count(value: Int) -> Int {
    let next = value
    return value + next
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    let highlights = document_highlight_via_request(
        &mut service,
        uri,
        offset_to_position(&source, nth_offset(&source, "value", 1)),
    )
    .await
    .expect("documentHighlight request should return current-file highlights");

    let highlight_ranges = highlights
        .into_iter()
        .map(|highlight| highlight.range)
        .collect::<Vec<_>>();
    let expected_ranges = vec![
        range_for(&source, "value", 1),
        range_for(&source, "value", 2),
        range_for(&source, "value", 3),
    ];
    assert_eq!(highlight_ranges, expected_ranges);
}
