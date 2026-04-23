use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ql_lsp::Backend;
use serde_json::json;
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::{Id, Request};
use tower_lsp::lsp_types::request::{GotoImplementationParams, GotoImplementationResponse};
use tower_lsp::lsp_types::{
    DidOpenTextDocumentParams, InitializeParams, Location, Position, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Url,
};
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
) -> Option<GotoImplementationResponse> {
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

struct WorkspaceRootRunnerFixture {
    _temp: TempDir,
    core_source: String,
    core_uri: Url,
    app_source: String,
    app_uri: Url,
    tools_source: String,
    tools_uri: Url,
}

fn setup_workspace_root_runner_fixture(prefix: &str) -> WorkspaceRootRunnerFixture {
    let temp = TempDir::new(prefix);
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
    );
    let tools_path = temp.write(
        "workspace/packages/tools/src/lib.ql",
        r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );

    WorkspaceRootRunnerFixture {
        _temp: temp,
        core_source: fs::read_to_string(&core_path).expect("core source should read"),
        core_uri: Url::from_file_path(&core_path).expect("core path should convert to URI"),
        app_source: fs::read_to_string(&app_path).expect("app source should read"),
        app_uri: Url::from_file_path(&app_path).expect("app path should convert to URI"),
        tools_source: fs::read_to_string(&tools_path).expect("tools source should read"),
        tools_uri: Url::from_file_path(&tools_path).expect("tools path should convert to URI"),
    }
}

struct WorkspaceTypeImportFixture {
    _temp: TempDir,
    app_source: String,
    app_uri: Url,
    core_source: String,
    core_uri: Url,
}

fn setup_workspace_type_import_fixture(
    prefix: &str,
    core_source: &str,
) -> WorkspaceTypeImportFixture {
    let temp = TempDir::new(prefix);
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
    );
    let core_path = temp.write("workspace/packages/core/src/lib.ql", core_source);
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

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
    );

    WorkspaceTypeImportFixture {
        _temp: temp,
        app_source: fs::read_to_string(&app_path).expect("app source should read"),
        app_uri: Url::from_file_path(&app_path).expect("app path should convert to URI"),
        core_source: fs::read_to_string(&core_path).expect("core source should read"),
        core_uri: Url::from_file_path(&core_path).expect("core path should convert to URI"),
    }
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
    .await
    .expect("dependency field type implementation should exist");
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
    .await
    .expect("matching broken open dependency implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken open dependency implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_task_source,
            nth_offset(&open_task_source, "extend Cfg", 1)
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_member_types_in_broken_source(
) {
    let temp =
        TempDir::new("ql-lsp-implementation-request-broken-same-named-dependency-member-types");
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
    .await
    .expect("broken-source dependency member type implementation should exist");
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

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_scalar_for_same_file_method_call() {
    let temp = TempDir::new("ql-lsp-implementation-request-same-file-method-call");
    let source_path = temp.write(
        "sample.ql",
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    return counter.read(1)
}
"#,
    );
    let source = fs::read_to_string(&source_path).expect("same-file source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "read", 2)),
    )
    .await
    .expect("same-file method call implementation should exist");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("same-file concrete method call should resolve to one implementation")
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range.start,
        offset_to_position(&source, nth_offset(&source, "read", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_none_for_same_file_method_definition_site() {
    let temp = TempDir::new("ql-lsp-implementation-request-same-file-declaration-site");
    let source_path = temp.write(
        "sample.ql",
        r#"
struct Config {
    value: Int,
}

impl Config {
    fn get(self) -> Int {
        return self.value
    }
}

fn read(config: Config) -> Int {
    return config.get()
}
"#,
    );
    let source = fs::read_to_string(&source_path).expect("same-file source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        uri,
        offset_to_position(&source, nth_offset(&source, "get", 1)),
    )
    .await;
    assert_eq!(implementation, None);
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_scalar_for_same_file_trait_surface() {
    let temp = TempDir::new("ql-lsp-implementation-request-same-file-trait-surface");
    let source_path = temp.write(
        "sample.ql",
        r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
    );
    let source = fs::read_to_string(&source_path).expect("same-file source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Runner", 1)),
    )
    .await
    .expect("same-file trait implementation should exist");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("single same-file trait implementation should resolve to one location")
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range.start,
        offset_to_position(&source, nth_offset(&source, "impl Runner for Worker", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_same_file_trait_method_definition() {
    let temp = TempDir::new("ql-lsp-implementation-request-same-file-trait-method-array");
    let source_path = temp.write(
        "sample.ql",
        r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}
struct Helper {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}

impl Runner for Helper {
    fn run(self) -> Int {
        return 2
    }
}
"#,
    );
    let source = fs::read_to_string(&source_path).expect("same-file source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "run", 1)),
    )
    .await
    .expect("same-file trait method implementations should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("same-file trait method definition should resolve to many locations")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations.iter().all(|location| location.uri == uri),
        "all same-file trait method implementations should stay in the current file",
    );
    for occurrence in [2usize, 3usize] {
        assert!(
            locations.iter().any(|location| {
                location.range.start
                    == offset_to_position(&source, nth_offset(&source, "run", occurrence))
            }),
            "same-file trait method implementations should include run occurrence {occurrence}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_same_file_type_surface() {
    let temp = TempDir::new("ql-lsp-implementation-request-same-file-type-surface");
    let source_path = temp.write(
        "sample.ql",
        r#"
struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#,
    );
    let source = fs::read_to_string(&source_path).expect("same-file source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Config", 1)),
    )
    .await
    .expect("same-file type implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("same-file type surface should resolve to many implementation blocks")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations.iter().all(|location| location.uri == uri),
        "all same-file type implementations should stay in the current file",
    );
    for marker in ["impl Config", "extend Config"] {
        assert!(
            locations.iter().any(|location| {
                location.range.start == offset_to_position(&source, nth_offset(&source, marker, 1))
            }),
            "same-file type implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_workspace_type_import_surface() {
    let fixture = setup_workspace_type_import_fixture(
        "ql-lsp-implementation-request-workspace-type-import-surface",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#,
    );
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        fixture.app_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.app_uri.clone(),
        offset_to_position(
            &fixture.app_source,
            nth_offset(&fixture.app_source, "Config", 2),
        ),
    )
    .await
    .expect("workspace type import implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("workspace type import surface should resolve to many implementation blocks")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations
            .iter()
            .all(|location| location.uri == fixture.core_uri),
        "all workspace type implementations should point at workspace source",
    );
    for marker in ["impl Config", "extend Config"] {
        assert!(
            locations.iter().any(|location| {
                location.range.start
                    == offset_to_position(
                        &fixture.core_source,
                        nth_offset(&fixture.core_source, marker, 1),
                    )
            }),
            "workspace type implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_open_workspace_source_for_workspace_type_import_surface() {
    let fixture = setup_workspace_type_import_fixture(
        "ql-lsp-implementation-request-open-workspace-type-import-surface",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}
"#,
    );
    let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn helper() -> Int {
    return 0
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#
    .to_owned();
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        open_core_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        fixture.app_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.app_uri.clone(),
        offset_to_position(
            &fixture.app_source,
            nth_offset(&fixture.app_source, "Config", 2),
        ),
    )
    .await
    .expect("workspace type import implementation should use open workspace source");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("workspace type import surface should resolve to many implementation blocks")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations
            .iter()
            .all(|location| location.uri == fixture.core_uri),
        "all open workspace type implementations should stay in the open source",
    );
    for marker in ["impl Config", "extend Config"] {
        assert!(
            locations.iter().any(|location| {
                location.range.start
                    == offset_to_position(
                        &open_core_source,
                        nth_offset(&open_core_source, marker, 1),
                    )
            }),
            "open workspace type implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_workspace_trait_import_surface() {
    let fixture =
        setup_workspace_root_runner_fixture("ql-lsp-implementation-request-workspace-trait-import");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        fixture.app_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.app_uri.clone(),
        offset_to_position(
            &fixture.app_source,
            nth_offset(&fixture.app_source, "Runner", 2),
        ),
    )
    .await
    .expect("workspace trait import implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("workspace trait import surface should resolve to many implementation blocks")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source, marker) in [
        (
            fixture.app_uri.clone(),
            fixture.app_source.as_str(),
            "impl Runner for AppWorker",
        ),
        (
            fixture.tools_uri.clone(),
            fixture.tools_source.as_str(),
            "impl Runner for ToolWorker",
        ),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, marker, 1))
            }),
            "workspace trait implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_source_for_workspace_trait_import() {
    let temp = TempDir::new("ql-lsp-implementation-request-broken-open-workspace-trait-import");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
    );
    let tools_path = temp.write(
        "workspace/packages/tools/src/lib.ql",
        r#"
package demo.tools

pub fn ready() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );

    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");
    let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}

pub fn broken() -> Int {
    return ToolWorker {
"#
    .to_owned();

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&app_source, nth_offset(&app_source, "Runner", 2)),
    )
    .await
    .expect("disk-only workspace trait implementation should exist");
    let GotoImplementationResponse::Scalar(location) = disk_only else {
        panic!("disk-only workspace trait import should resolve to one implementation")
    };
    assert_eq!(location.uri, app_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(
            &app_source,
            nth_offset(&app_source, "impl Runner for AppWorker", 1)
        ),
    );

    did_open_via_request(&mut service, tools_uri.clone(), open_tools_source.clone()).await;
    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&app_source, nth_offset(&app_source, "Runner", 2)),
    )
    .await
    .expect("workspace trait implementation should use broken open workspace source");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken open workspace trait import should resolve to many implementations")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations.iter().any(|location| {
            location.uri == app_uri
                && location.range.start
                    == offset_to_position(
                        &app_source,
                        nth_offset(&app_source, "impl Runner for AppWorker", 1),
                    )
        }),
        "workspace trait implementations should keep the current app implementation",
    );
    assert!(
        locations.iter().any(|location| {
            location.uri == tools_uri
                && location.range.start
                    == offset_to_position(
                        &open_tools_source,
                        nth_offset(&open_tools_source, "impl Runner for ToolWorker", 1),
                    )
        }),
        "workspace trait implementations should include the broken open workspace implementation",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_workspace_root_trait_surface() {
    let fixture =
        setup_workspace_root_runner_fixture("ql-lsp-implementation-request-workspace-root-trait");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        fixture.app_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.tools_uri.clone(),
        fixture.tools_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.core_uri.clone(),
        offset_to_position(
            &fixture.core_source,
            nth_offset(&fixture.core_source, "Runner", 1),
        ),
    )
    .await
    .expect("workspace root trait implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("workspace root trait surface should resolve to many implementation blocks")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source, marker) in [
        (
            fixture.app_uri.clone(),
            fixture.app_source.as_str(),
            "impl Runner for AppWorker",
        ),
        (
            fixture.tools_uri.clone(),
            fixture.tools_source.as_str(),
            "impl Runner for ToolWorker",
        ),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, marker, 1))
            }),
            "workspace root trait implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_workspace_root_trait_method_definition() {
    let fixture = setup_workspace_root_runner_fixture(
        "ql-lsp-implementation-request-workspace-root-trait-method-definition",
    );
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        fixture.app_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.tools_uri.clone(),
        fixture.tools_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.core_uri.clone(),
        offset_to_position(
            &fixture.core_source,
            nth_offset(&fixture.core_source, "run", 1),
        ),
    )
    .await
    .expect("workspace root trait method implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("workspace root trait method definition should resolve to many implementations")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source) in [
        (fixture.app_uri.clone(), fixture.app_source.as_str()),
        (fixture.tools_uri.clone(), fixture.tools_source.as_str()),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, "run", 1))
            }),
            "workspace root trait method implementations should include {uri}",
        );
    }
}
