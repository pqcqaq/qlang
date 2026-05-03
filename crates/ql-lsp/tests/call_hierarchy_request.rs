mod common;

use common::request::{
    TempDir, incoming_calls_via_request, initialized_service_with_open_documents, nth_offset,
    offset_to_position, outgoing_calls_via_request, prepare_call_hierarchy_via_request,
};

#[tokio::test]
async fn call_hierarchy_request_reports_same_file_function_calls() {
    let temp = TempDir::new("ql-lsp-call-hierarchy-functions");
    let source = r#"
fn leaf(value: Int) -> Int {
    return value
}

fn caller(value: Int) -> Int {
    return leaf(value)
}

fn caller_twice(value: Int) -> Int {
    let first = leaf(value)
    return leaf(first)
}
"#;
    let path = temp.write("calls.ql", source);
    let uri = tower_lsp::lsp_types::Url::from_file_path(&path).expect("uri should be valid");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let leaf_position = offset_to_position(source, nth_offset(source, "leaf", 1));
    let items = prepare_call_hierarchy_via_request(&mut service, uri.clone(), leaf_position)
        .await
        .expect("prepareCallHierarchy should return leaf");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "leaf");

    let incoming = incoming_calls_via_request(&mut service, items[0].clone())
        .await
        .expect("incomingCalls should return callers");
    assert_eq!(incoming.len(), 2);
    assert_eq!(incoming[0].from.name, "caller");
    assert_eq!(incoming[0].from_ranges.len(), 1);
    assert_eq!(incoming[1].from.name, "caller_twice");
    assert_eq!(incoming[1].from_ranges.len(), 2);

    let caller_position = offset_to_position(source, nth_offset(source, "caller", 1));
    let caller_items =
        prepare_call_hierarchy_via_request(&mut service, uri.clone(), caller_position)
            .await
            .expect("prepareCallHierarchy should return caller");
    let outgoing = outgoing_calls_via_request(&mut service, caller_items[0].clone())
        .await
        .expect("outgoingCalls should return callees");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to.name, "leaf");
    assert_eq!(outgoing[0].from_ranges.len(), 1);
}

#[tokio::test]
async fn call_hierarchy_request_reports_same_file_method_calls() {
    let temp = TempDir::new("ql-lsp-call-hierarchy-methods");
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn bump(self) -> Int {
        return self.get()
    }
}
"#;
    let path = temp.write("methods.ql", source);
    let uri = tower_lsp::lsp_types::Url::from_file_path(&path).expect("uri should be valid");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let get_position = offset_to_position(source, nth_offset(source, "get", 1));
    let get_items = prepare_call_hierarchy_via_request(&mut service, uri.clone(), get_position)
        .await
        .expect("prepareCallHierarchy should return method");
    assert_eq!(get_items[0].name, "get");

    let incoming = incoming_calls_via_request(&mut service, get_items[0].clone())
        .await
        .expect("incomingCalls should return method callers");
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from.name, "bump");
    assert_eq!(incoming[0].from_ranges.len(), 1);

    let bump_position = offset_to_position(source, nth_offset(source, "bump", 1));
    let bump_items = prepare_call_hierarchy_via_request(&mut service, uri, bump_position)
        .await
        .expect("prepareCallHierarchy should return caller method");
    let outgoing = outgoing_calls_via_request(&mut service, bump_items[0].clone())
        .await
        .expect("outgoingCalls should return method callees");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to.name, "get");
    assert_eq!(outgoing[0].from_ranges.len(), 1);
}
