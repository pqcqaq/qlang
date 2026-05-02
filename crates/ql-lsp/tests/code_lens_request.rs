mod common;

use common::request::{
    TempDir, code_lens_resolve_via_request, code_lens_via_request,
    initialized_service_with_open_documents, nth_offset, offset_to_position,
};
use tower_lsp::lsp_types::Url;

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_returns_references_and_implementation_lenses() {
    let temp = TempDir::new("ql-lsp-code-lens-request");
    let source = r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}

fn helper() -> Int {
    return 2
}

fn main(worker: Worker) -> Int {
    return helper() + worker.run()
}
"#;
    let source_path = temp.write("sample.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let lenses = code_lens_via_request(&mut service, uri)
        .await
        .expect("codeLens request should return lenses");

    let helper_position = offset_to_position(source, nth_offset(source, "helper", 1));
    let helper_lens = lenses
        .iter()
        .find(|lens| {
            lens.range.start == helper_position
                && lens.command.as_ref().is_some_and(|command| {
                    command.title == "1 reference"
                        && command.command == "editor.action.showReferences"
                })
        })
        .unwrap_or_else(|| panic!("helper reference code lens should exist: {lenses:#?}"));
    assert_eq!(
        helper_lens
            .command
            .as_ref()
            .and_then(|command| command.arguments.as_ref())
            .map(Vec::len),
        Some(3),
    );

    let runner_position = offset_to_position(source, nth_offset(source, "Runner", 1));
    assert!(
        lenses.iter().any(|lens| {
            lens.range.start == runner_position
                && lens.command.as_ref().is_some_and(|command| {
                    command.title == "1 implementation"
                        && command.command == "editor.action.showReferences"
                })
        }),
        "trait implementation code lens should exist: {lenses:#?}",
    );

    let resolved = code_lens_resolve_via_request(&mut service, helper_lens.clone()).await;
    assert_eq!(resolved, *helper_lens);
}

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_returns_none_for_parse_errors() {
    let temp = TempDir::new("ql-lsp-code-lens-request-parse-error");
    let source = "fn broken( -> Int {\n    return 1\n}\n";
    let source_path = temp.write("broken.ql", source);
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.to_owned())]).await;

    let lenses = code_lens_via_request(&mut service, uri).await;

    assert_eq!(lenses, None);
}
