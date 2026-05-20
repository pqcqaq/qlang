use std::fs;

use crate::common::request::{
    TempDir, code_lens_resolve_via_request, code_lens_via_request, did_open_via_request,
    nth_offset, offset_to_position,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::open_real_stdlib_workspace;
use tower_lsp::lsp_types::{CodeLens, Location, Position, Url};

#[tokio::test(flavor = "current_thread")]
async fn code_lens_request_uses_current_real_stdlib_workspace() {
    let temp = TempDir::new("ql-lsp-real-stdlib-code-lens-request");
    let app_source = r#"
package demo.app

use std.core.max_int as max_int

trait Runner {
    fn run(self) -> Int
}

struct Worker {}

impl Runner for Worker {
    fn run(self) -> Int {
        return max_int(1, 2)
    }
}

pub fn main(worker: Worker) -> Int {
    return worker.run()
}
"#;
    let (mut service, app_uri, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;

    let app_lenses = code_lens_via_request(&mut service, app_uri.clone())
        .await
        .expect("real stdlib app codeLens request should return source lenses");
    let runner_position = offset_to_position(app_source, nth_offset(app_source, "Runner", 1));
    let implementation_lens =
        code_lens_at(&app_lenses, runner_position, &["implementation"]).clone();
    let implementation_locations = code_lens_locations(&implementation_lens);
    assert!(
        implementation_locations.iter().any(|location| {
            location.uri == app_uri
                && location.range.start
                    == offset_to_position(app_source, nth_offset(app_source, "impl Runner", 1))
        }),
        "real stdlib app codeLens should link local trait implementations: {implementation_locations:#?}",
    );
    let resolved = code_lens_resolve_via_request(&mut service, implementation_lens.clone()).await;
    assert_eq!(resolved, implementation_lens);

    let core_path = real_stdlib_source_path(&stdlib_root, "core");
    let core_source = fs::read_to_string(&core_path)
        .expect("temp std.core source should exist")
        .replace("\r\n", "\n");
    let core_uri =
        Url::from_file_path(&core_path).expect("temp std.core source path should convert to URI");
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;

    let core_lenses = code_lens_via_request(&mut service, core_uri)
        .await
        .expect("real stdlib source codeLens request should return dependency source lenses");
    let max_int_position = offset_to_position(&core_source, nth_offset(&core_source, "max_int", 1));
    let reference_lens = code_lens_at(&core_lenses, max_int_position, &["reference", "references"]);
    let reference_locations = code_lens_locations(reference_lens);
    assert!(
        reference_locations.iter().any(|location| {
            location.uri == app_uri
                && location.range.start
                    == offset_to_position(app_source, nth_offset(app_source, "max_int", 3))
        }),
        "real stdlib source codeLens should count app consumer references: {reference_locations:#?}",
    );
}

fn code_lens_at<'a>(
    lenses: &'a [CodeLens],
    position: Position,
    title_suffixes: &[&str],
) -> &'a CodeLens {
    lenses
        .iter()
        .find(|lens| {
            lens.range.start == position
                && lens.command.as_ref().is_some_and(|command| {
                    command.command == "editor.action.showReferences"
                        && title_suffixes
                            .iter()
                            .any(|suffix| command.title.ends_with(suffix))
                })
        })
        .unwrap_or_else(|| {
            panic!("codeLens should exist at {position:?} with title suffix {title_suffixes:?}: {lenses:#?}")
        })
}

fn code_lens_locations(lens: &CodeLens) -> Vec<Location> {
    lens.command
        .as_ref()
        .and_then(|command| command.arguments.as_ref())
        .and_then(|arguments| arguments.get(2).cloned())
        .and_then(|value| serde_json::from_value::<Vec<Location>>(value).ok())
        .unwrap_or_else(|| panic!("codeLens command should carry reference locations: {lens:#?}"))
}
