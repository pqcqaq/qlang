use std::fs;

use crate::common::request::{
    TempDir, did_open_via_request, incoming_calls_via_request, nth_offset, offset_to_position,
    outgoing_calls_via_request, prepare_call_hierarchy_via_request,
    prepare_type_hierarchy_via_request,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::{assert_call_hierarchy_targets, open_real_stdlib_workspace};
use tower_lsp::lsp_types::{SymbolKind, Url};

#[tokio::test(flavor = "current_thread")]
async fn hierarchy_requests_use_current_real_stdlib_sources() {
    let temp = TempDir::new("ql-lsp-real-stdlib-hierarchy-requests");
    let app_source = r#"
package demo.app

use std.core.clamp_bounds_int as clamp_bounds_int
use std.option.Option as Option

pub fn main() -> Int {
    return clamp_bounds_int(42, 0, 100)
}
"#;
    let (mut service, _, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;
    let core_source_path = real_stdlib_source_path(&stdlib_root, "core");
    let core_source = fs::read_to_string(&core_source_path)
        .expect("temp std.core source should exist")
        .replace("\r\n", "\n");
    let core_uri =
        Url::from_file_path(&core_source_path).expect("std.core source path should convert to URI");
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;

    let clamp_bounds_items = prepare_call_hierarchy_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset(&core_source, "clamp_bounds_int", 1),
        ),
    )
    .await
    .expect("real std.core prepareCallHierarchy should return clamp_bounds_int");
    assert_eq!(clamp_bounds_items.len(), 1);
    assert_eq!(clamp_bounds_items[0].name, "clamp_bounds_int");

    let outgoing = outgoing_calls_via_request(&mut service, clamp_bounds_items[0].clone())
        .await
        .expect("real std.core outgoingCalls should return callees");
    assert_call_hierarchy_targets(&outgoing, &["clamp_int", "max_int", "min_int"]);

    let clamp_int_items = prepare_call_hierarchy_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(&core_source, nth_offset(&core_source, "clamp_int", 1)),
    )
    .await
    .expect("real std.core prepareCallHierarchy should return clamp_int");
    let incoming = incoming_calls_via_request(&mut service, clamp_int_items[0].clone())
        .await
        .expect("real std.core incomingCalls should return callers");
    assert!(
        incoming
            .iter()
            .any(|call| call.from.name == "clamp_bounds_int" && call.from_ranges.len() == 1),
        "clamp_int incomingCalls should include clamp_bounds_int: {incoming:#?}",
    );

    let option_source_path = real_stdlib_source_path(&stdlib_root, "option");
    let option_source = fs::read_to_string(&option_source_path)
        .expect("temp std.option source should exist")
        .replace("\r\n", "\n");
    let option_uri = Url::from_file_path(&option_source_path)
        .expect("std.option source path should convert to URI");
    did_open_via_request(&mut service, option_uri.clone(), option_source.clone()).await;
    let option_items = prepare_type_hierarchy_via_request(
        &mut service,
        option_uri,
        offset_to_position(&option_source, nth_offset(&option_source, "Option", 1)),
    )
    .await
    .expect("real std.option prepareTypeHierarchy should return Option");
    assert_eq!(option_items.len(), 1);
    assert_eq!(option_items[0].name, "Option");
    assert_eq!(option_items[0].kind, SymbolKind::ENUM);
}
