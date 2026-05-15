use crate::common::request::{
    TempDir, goto_implementation_via_request, nth_offset, offset_to_position,
};
use crate::support::open_real_stdlib_workspace;
use tower_lsp::lsp_types::Location;
use tower_lsp::lsp_types::request::GotoImplementationResponse;

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_current_real_stdlib_workspace_local_type_surface() {
    let temp = TempDir::new("ql-lsp-real-stdlib-implementation-request");
    let app_source = r#"
package demo.app

use std.core.max_int as largest_int

pub struct Worker {
    value: Int,
}

impl Worker {
    fn read(self) -> Int {
        return self.value
    }
}

pub fn main(worker: Worker) -> Int {
    let limit = largest_int(1, 2)
    return worker.read() + limit
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&app_source, nth_offset(app_source, "Worker", 3)),
    )
    .await
    .expect("real stdlib workspace implementation should return a source location");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!(
            "real stdlib workspace implementation should resolve to one location: {implementation:#?}"
        )
    };
    assert_eq!(uri, app_uri);
    assert_eq!(
        range.start,
        offset_to_position(&app_source, nth_offset(app_source, "impl Worker", 1)),
    );
}
