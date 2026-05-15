use crate::common::request::{TempDir, completion_resolve_via_request};
use crate::support::{
    assert_contains_all, assert_not_contains_any, completion_at, completion_documentation,
    completion_items, completion_labels, open_real_stdlib_workspace,
};

#[tokio::test(flavor = "current_thread")]
async fn completion_request_uses_current_real_stdlib_surface() {
    let temp = TempDir::new("ql-lsp-real-stdlib-completion-request");
    let app_source = r#"
package demo.app

use std.core.
use std.option.
use std.result.
use std.array.
use std.test.

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;

    let core = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.core.").await,
    );
    assert_contains_all(&core, &["max_int", "is_even_int", "not_bool"]);

    let option = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.option.").await,
    );
    assert_contains_all(&option, &["Option", "some", "none_option"]);
    assert_not_contains_any(&option, &["IntOption", "some_int"]);

    let result = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.result.").await,
    );
    assert_contains_all(&result, &["Result", "ok", "ok_or"]);
    assert_not_contains_any(&result, &["IntResult", "ok_int"]);

    let array = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.array.").await,
    );
    assert_contains_all(&array, &["repeat_array", "sum_int_array", "reverse_array"]);

    let test =
        completion_labels(completion_at(&mut service, app_uri, app_source, "std.test.").await);
    assert_contains_all(
        &test,
        &["expect_eq", "expect_option_some", "expect_result_ok"],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn completion_resolve_restores_current_real_stdlib_symbol_docs() {
    let temp = TempDir::new("ql-lsp-real-stdlib-completion-resolve");
    let app_source = r#"
package demo.app

use std.option.

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;

    let items =
        completion_items(completion_at(&mut service, app_uri, app_source, "std.option.").await);
    let some = items
        .into_iter()
        .find(|item| item.label == "some")
        .expect("real std.option.some completion item should exist");
    let original_detail = some
        .detail
        .clone()
        .expect("real std.option.some should carry inline detail");
    let original_documentation = completion_documentation(&some);
    assert!(
        original_detail.contains("fn some") && original_detail.contains("Option"),
        "completion should be for the real std.option.some function: {some:#?}",
    );
    assert!(
        some.data.is_some(),
        "completion resolve should have source data to restore stripped docs: {some:#?}",
    );

    let mut stripped_some = some;
    stripped_some.detail = None;
    stripped_some.documentation = None;

    let resolved_some = completion_resolve_via_request(&mut service, stripped_some).await;
    assert_eq!(
        resolved_some.detail.as_deref(),
        Some(original_detail.as_str())
    );
    assert_eq!(
        completion_documentation(&resolved_some),
        original_documentation
    );
}
