mod common;

use common::request::{
    TempDir, completion_via_request, did_open_via_request, hover_via_request,
    initialize_service_with_workspace_roots, nth_offset, offset_to_position,
};
use common::stdlib_compat::{
    assert_compat_completion, assert_recommended_completion, completion_item, completion_items,
    write_stdlib_compat_workspace,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::{HoverContents, Url};

async fn open_stdlib_compat_workspace(
    temp: &TempDir,
    app_source: &str,
) -> (LspService<Backend>, Url) {
    let app_uri = write_stdlib_compat_workspace(temp, app_source);
    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;
    (service, app_uri)
}

#[tokio::test(flavor = "current_thread")]
async fn completion_request_marks_stdlib_compat_imports_deprecated() {
    let temp = TempDir::new("ql-lsp-stdlib-compat-completion-request");
    let app_source = r#"
package demo.app

use std.option.
use std.result.
use std.array.

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, app_uri) = open_stdlib_compat_workspace(&temp, app_source).await;

    let option_completion = completion_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            app_source,
            nth_offset(app_source, "std.option.", 1) + "std.option.".len(),
        ),
    )
    .await
    .expect("std.option completion request should return items");
    let option_items = completion_items(option_completion);
    assert_recommended_completion(completion_item(&option_items, "Option"));
    assert_recommended_completion(completion_item(&option_items, "some"));
    assert_compat_completion(completion_item(&option_items, "IntOption"));
    assert_compat_completion(completion_item(&option_items, "some_int"));

    let result_completion = completion_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            app_source,
            nth_offset(app_source, "std.result.", 1) + "std.result.".len(),
        ),
    )
    .await
    .expect("std.result completion request should return items");
    let result_items = completion_items(result_completion);
    assert_recommended_completion(completion_item(&result_items, "Result"));
    assert_recommended_completion(completion_item(&result_items, "ok"));
    assert_compat_completion(completion_item(&result_items, "IntResult"));
    assert_compat_completion(completion_item(&result_items, "ok_int"));

    let array_completion = completion_via_request(
        &mut service,
        app_uri,
        offset_to_position(
            app_source,
            nth_offset(app_source, "std.array.", 1) + "std.array.".len(),
        ),
    )
    .await
    .expect("std.array completion request should return items");
    let array_items = completion_items(array_completion);
    assert_recommended_completion(completion_item(&array_items, "sum_int_array"));
    assert_recommended_completion(completion_item(&array_items, "reverse_array"));
}

#[tokio::test(flavor = "current_thread")]
async fn hover_request_marks_stdlib_compat_imports_deprecated() {
    let temp = TempDir::new("ql-lsp-stdlib-compat-hover-request");
    let app_source = r#"
package demo.app

use std.option.IntOption as MaybeInt
use std.option.Option as GenericOption
use std.array.reverse_array as reverse_any

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, app_uri) = open_stdlib_compat_workspace(&temp, app_source).await;

    let compat_hover = hover_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "MaybeInt", 1)),
    )
    .await
    .expect("compat stdlib import hover request should return markup");
    let HoverContents::Markup(compat_markup) = compat_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(compat_markup.value.contains("**enum** `IntOption`"));
    assert!(compat_markup.value.contains("Compatibility API"));
    assert!(compat_markup.value.contains("Option[T]"));

    let recommended_hover = hover_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "GenericOption", 1)),
    )
    .await
    .expect("recommended stdlib import hover request should return markup");
    let HoverContents::Markup(recommended_markup) = recommended_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(recommended_markup.value.contains("**enum** `Option`"));
    assert!(!recommended_markup.value.contains("Compatibility API"));

    let array_hover = hover_via_request(
        &mut service,
        app_uri,
        offset_to_position(app_source, nth_offset(app_source, "reverse_any", 1)),
    )
    .await
    .expect("recommended array import hover request should return markup");
    let HoverContents::Markup(array_markup) = array_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(array_markup.value.contains("**function** `reverse_array`"));
    assert!(!array_markup.value.contains("Compatibility API"));
}
