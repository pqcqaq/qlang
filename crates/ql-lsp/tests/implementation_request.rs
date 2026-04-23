use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ql_lsp::Backend;
use serde_json::json;
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::{Id, Request};
use tower_lsp::lsp_types::{
    DidOpenTextDocumentParams, InitializeParams, Location, Position, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Url,
};
use tower_lsp::lsp_types::request::{GotoImplementationParams, GotoImplementationResponse};
use tower_lsp::{LanguageServer, LspService};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, _)| start)
        .expect("needle occurrence should exist")
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Position::new(line, prefix[line_start..].chars().count() as u32)
}

async fn initialize_service(service: &mut LspService<Backend>) {
    let request = Request::build("initialize")
        .params(json!(InitializeParams {
            ..InitializeParams::default()
        }))
        .id(1)
        .finish();
    let response = service
        .ready()
        .await
        .expect("service should become ready for initialize")
        .call(request)
        .await
        .expect("initialize request should succeed");
    let response = response.expect("initialize should return a response");
    assert_eq!(response.id(), &Id::Number(1));
    assert!(response.is_ok(), "initialize should succeed: {response:?}");
}

async fn did_open_via_request(service: &mut LspService<Backend>, uri: Url, text: String) {
    let request = Request::build("textDocument/didOpen")
        .params(json!(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "ql".to_owned(),
                version: 1,
                text,
            },
        }))
        .finish();
    let response = service
        .ready()
        .await
        .expect("service should become ready for didOpen")
        .call(request)
        .await
        .expect("didOpen notification should succeed");
    assert_eq!(response, None);
}

async fn goto_implementation_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> GotoImplementationResponse {
    let request = Request::build("textDocument/implementation")
        .params(json!(GotoImplementationParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }))
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .expect("service should become ready for gotoImplementation")
        .call(request)
        .await
        .expect("gotoImplementation request should succeed")
        .expect("gotoImplementation should return a response");
    assert_eq!(response.id(), &Id::Number(2));
    let result = response
        .result()
        .cloned()
        .expect("gotoImplementation should succeed");
    serde_json::from_value(result).expect("gotoImplementation result should deserialize")
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_open_local_dependency_member_types() {
    let temp = TempDir::new("ql-lsp-implementation-request-open-member-types");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}
"#,
    );
    let alpha_source_path = temp.write(
        "workspace/vendor/alpha/src/lib.ql",
        r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = { path = "../../vendor/alpha" }
"#,
    );
    temp.write(
        "workspace/vendor/alpha/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/alpha/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
    );

    let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

impl Extra {
    pub fn read(self) -> Int {
        return self.id
    }
}

extend Extra {
    pub fn bonus(self) -> Int {
        return self.id + 1
    }
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let alpha_uri =
        Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    service
        .inner()
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: alpha_uri.clone(),
                language_id: "ql".to_owned(),
                version: 1,
                text: open_alpha_source.clone(),
            },
        })
        .await;
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset(&source, "extra", 1) + 1),
    )
    .await;
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!(
            "dependency field type implementation should resolve to impl block locations: {implementation:?}"
        )
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations.iter().all(|location| location.uri == alpha_uri),
        "implementation should stay in the open dependency source",
    );
    for marker in ["impl Extra", "extend Extra"] {
        assert!(
            locations.iter().any(|location| {
                location.range.start
                    == offset_to_position(
                        &open_alpha_source,
                        nth_offset(&open_alpha_source, marker, 1),
                    )
            }),
            "implementation should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_filters_same_named_local_dependency_in_broken_open_consumer() {
    let temp = TempDir::new("ql-lsp-implementation-request-broken-same-named-local-dependency");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return 0
}
"#,
    );
    let task_path = temp.write(
        "workspace/packages/app/src/task.ql",
        r#"
package demo.app

pub fn task() -> Int {
    return 0
}
"#,
    );
    temp.write(
        "workspace/vendor/alpha/src/lib.ql",
        r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub struct Config {
    value: Bool,
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = { path = "../../vendor/alpha" }
beta = { path = "../../vendor/beta" }
"#,
    );
    temp.write(
        "workspace/vendor/alpha/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/beta/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/alpha/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.beta

pub struct Config {
    value: Bool,
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Config as Cfg
use demo.shared.beta.Config as OtherCfg

extend Cfg {
    fn alpha(self) -> Int {
        return 1
    }
}

extend OtherCfg {
    fn beta(self) -> Bool {
        return true
    }
}

pub fn broken() -> Int {
    return Cfg {
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    service
        .inner()
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: task_uri.clone(),
                language_id: "ql".to_owned(),
                version: 1,
                text: open_task_source.clone(),
            },
        })
        .await;
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
    )
    .await;
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken open dependency implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(&open_task_source, nth_offset(&open_task_source, "extend Cfg", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_member_types_in_broken_source(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-same-named-dependency-member-types",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    let current = build()
    let next = other()
    return current.extra.id + next.extra.flag
"#,
    );
    let alpha_source_path = temp.write(
        "workspace/vendor/alpha/src/lib.ql",
        r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub struct Extra {
    flag: Bool,
}

impl Extra {
    pub fn read(self) -> Bool {
        return self.flag
    }
}

extend Extra {
    pub fn bonus(self) -> Bool {
        return self.flag
    }
}

pub struct Counter {
    extra: Extra,
}

pub fn build() -> Counter {
    return Counter { extra: Extra { flag: true } }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = { path = "../../vendor/alpha" }
beta = { path = "../../vendor/beta" }
"#,
    );
    temp.write(
        "workspace/vendor/alpha/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/beta/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/alpha/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

pub fn build() -> Counter
"#,
    );
    temp.write(
        "workspace/vendor/beta/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.beta

pub struct Extra {
    flag: Bool,
}

impl Extra {
    pub fn read(self) -> Bool
}

extend Extra {
    pub fn bonus(self) -> Bool
}

pub struct Counter {
    extra: Extra,
}

pub fn build() -> Counter
"#,
    );

    let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

impl Extra {
    pub fn read(self) -> Int {
        return self.id
    }
}

extend Extra {
    pub fn bonus(self) -> Int {
        return self.id + 1
    }
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let alpha_uri =
        Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    service
        .inner()
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: alpha_uri.clone(),
                language_id: "ql".to_owned(),
                version: 1,
                text: open_alpha_source.clone(),
            },
        })
        .await;
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset(&source, "extra", 1) + 1),
    )
    .await;
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!(
            "broken-source dependency member type implementation should resolve to impl block locations: {implementation:?}"
        )
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations.iter().all(|location| location.uri == alpha_uri),
        "implementation should stay in the matching alpha dependency source",
    );
    for marker in ["impl Extra", "extend Extra"] {
        assert!(
            locations.iter().any(|location| {
                location.range.start
                    == offset_to_position(
                        &open_alpha_source,
                        nth_offset(&open_alpha_source, marker, 1),
                    )
            }),
            "implementation should include alpha {marker}",
        );
    }
}
