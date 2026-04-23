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

fn nth_offset_in_context(source: &str, needle: &str, context: &str, occurrence: usize) -> usize {
    let context_start = nth_offset(source, context, occurrence);
    let relative = context
        .match_indices(needle)
        .last()
        .map(|(start, _)| start)
        .expect("needle should exist inside context");
    context_start + relative
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

struct WorkspaceRootTraitSingleConsumerFixture {
    _temp: TempDir,
    core_source: String,
    core_uri: Url,
    app_source: String,
    app_uri: Url,
}

fn setup_workspace_root_trait_single_consumer_fixture(
    prefix: &str,
    app_source: &str,
) -> WorkspaceRootTraitSingleConsumerFixture {
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

    WorkspaceRootTraitSingleConsumerFixture {
        _temp: temp,
        core_source: fs::read_to_string(&core_path).expect("core source should read"),
        core_uri: Url::from_file_path(&core_path).expect("core path should convert to URI"),
        app_source: fs::read_to_string(&app_path).expect("app source should read"),
        app_uri: Url::from_file_path(&app_path).expect("app path should convert to URI"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_locations_for_workspace_dependency_non_import_positions()
{
    let temp =
        TempDir::new("ql-lsp-implementation-request-workspace-dependency-non-import-positions");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.Holder as Hold

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let holder = Hold { child: config.clone_self() }
    let command = Cmd.Retry(1)
    return holder.child.value + built.value + command.unwrap_or(0)
}
"#,
    );
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

impl Config {
    pub fn clone_self(self) -> Config {
        return self
    }
}

extend Config {
    pub fn extra(self) -> Int {
        return self.limit
    }
}
"#,
    );
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

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

impl Config {
    pub fn clone_self(self) -> Config
}
"#,
    );

    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;

    for (needle, occurrence, expected_markers) in [
        ("built", 2usize, &["impl Config", "extend Config"][..]),
        ("Retry", 1usize, &["impl Command"][..]),
        ("child", 2usize, &["impl Config", "extend Config"][..]),
        ("clone_self", 1usize, &["impl Config", "extend Config"][..]),
    ] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&app_source, nth_offset(&app_source, needle, occurrence)),
        )
        .await
        .unwrap_or_else(|| panic!("workspace dependency implementation should exist for {needle}"));
        let locations = match implementation {
            GotoImplementationResponse::Scalar(location) => vec![location],
            GotoImplementationResponse::Array(locations) => locations,
            GotoImplementationResponse::Link(_) => {
                panic!("workspace dependency implementation should resolve to locations")
            }
        };
        assert_eq!(
            locations.len(),
            expected_markers.len(),
            "workspace dependency implementation should return all source impl blocks for {needle}",
        );
        assert!(
            locations.iter().all(|location| location.uri == core_uri),
            "workspace dependency implementation should stay in the core source for {needle}",
        );
        for marker in expected_markers {
            assert!(
                locations.iter().any(|location| {
                    location.range.start
                        == offset_to_position(&core_source, nth_offset(&core_source, marker, 1))
                }),
                "workspace dependency implementation should include {marker} for {needle}",
            );
        }
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

    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1),
        )
        .await
        .unwrap_or_else(|| panic!("dependency member type implementation should exist for {needle}"));
        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!(
                "dependency member type implementation should resolve to impl block locations: {implementation:?}"
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
                "implementation should include {marker} for {needle}",
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_open_local_dependency_member_types_in_broken_source() {
    let temp = TempDir::new("ql-lsp-implementation-request-open-member-types-broken-source");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
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
    assert!(
        ql_analysis::analyze_source(&source).is_err(),
        "current source should stay broken for this regression",
    );
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let alpha_uri =
        Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let disk_only = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1),
        )
        .await;
        assert_eq!(
            disk_only, None,
            "disk-only broken-source implementation should miss unsaved dependency member type {needle}",
        );
    }

    did_open_via_request(&mut service, alpha_uri.clone(), open_alpha_source.clone()).await;
    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1),
        )
        .await
        .expect("broken-source dependency member type implementation should use open source");
        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!(
                "broken-source dependency member type implementation should resolve to impl block locations: {implementation:?}"
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
                "broken-source dependency member type implementation should include {marker} for {needle}",
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_open_local_dependency_method_call() {
    let temp = TempDir::new("ql-lsp-implementation-request-open-local-dependency-method-call");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pulse()
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

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let alpha_uri =
        Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
    let pulse_position = offset_to_position(&source, nth_offset(&source, "pulse", 1) + 1);

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    assert_eq!(
        goto_implementation_via_request(&mut service, app_uri.clone(), pulse_position).await,
        None,
        "disk-only implementation should miss unsaved dependency method call",
    );

    did_open_via_request(&mut service, alpha_uri.clone(), open_alpha_source.clone()).await;
    let implementation = goto_implementation_via_request(&mut service, app_uri, pulse_position)
        .await
        .expect("dependency method call implementation should use open source");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("dependency concrete method call should resolve to one implementation")
    };
    assert_eq!(location.uri, alpha_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(&open_alpha_source, nth_offset(&open_alpha_source, "pulse", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_open_local_dependency_method_call_in_broken_source() {
    let temp =
        TempDir::new("ql-lsp-implementation-request-open-local-dependency-method-call-broken");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pulse(
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

pub struct Counter {
    value: Int,
}

pub fn marker() -> Int {
    return 1
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    assert!(
        ql_analysis::analyze_source(&source).is_err(),
        "current source should stay broken for this regression",
    );
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let alpha_uri =
        Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
    let pulse_position = offset_to_position(&source, nth_offset(&source, "pulse", 1) + 1);

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    assert_eq!(
        goto_implementation_via_request(&mut service, app_uri.clone(), pulse_position).await,
        None,
        "disk-only broken-source implementation should miss unsaved dependency method call",
    );

    did_open_via_request(&mut service, alpha_uri.clone(), open_alpha_source.clone()).await;
    let implementation = goto_implementation_via_request(&mut service, app_uri, pulse_position)
        .await
        .expect("broken-source dependency method call implementation should use open source");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("broken-source dependency concrete method call should resolve to one implementation")
    };
    assert_eq!(location.uri, alpha_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(&open_alpha_source, nth_offset(&open_alpha_source, "pulse", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_workspace_dependency_trait_method_call() {
    let temp =
        TempDir::new("ql-lsp-implementation-request-workspace-dependency-trait-method-call");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
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
    let bots_path = temp.write(
        "workspace/packages/bots/src/lib.ql",
        r#"
package demo.bots

use demo.core.Runner

struct BotWorker {}

impl Runner for BotWorker {
    fn run(self) -> Int {
        return 3
    }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/bots", "packages/core", "packages/tools"]
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
        "workspace/packages/bots/qlang.toml",
        r#"
[package]
name = "bots"

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
        "workspace/packages/tools/qlang.toml",
        r#"
[package]
name = "tools"

[references]
packages = ["../core"]
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
    let tools_source = fs::read_to_string(&tools_path).expect("tools source should read");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");
    let bots_source = fs::read_to_string(&bots_path).expect("bots source should read");
    let bots_uri = Url::from_file_path(&bots_path).expect("bots path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&app_source, nth_offset(&app_source, "run()", 1)),
    )
    .await
    .expect("dependency trait method call implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("dependency trait method call should resolve to many implementation methods")
    };

    assert_eq!(locations.len(), 2);
    for (uri, source) in [(&tools_uri, &tools_source), (&bots_uri, &bots_source)] {
        assert!(
            locations.iter().any(|location| {
                location.uri == *uri
                    && location.range.start
                        == offset_to_position(
                            source,
                            nth_offset_in_context(source, "run", "fn run(self)", 1),
                        )
            }),
            "dependency trait method call should include {uri}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_open_workspace_source_for_dependency_trait_method_call() {
    let temp =
        TempDir::new("ql-lsp-implementation-request-open-workspace-dependency-trait-method-call");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
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
    let tools_path = temp.write(
        "workspace/packages/tools/src/lib.ql",
        r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn walk(self) -> Int {
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
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
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

    let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#
    .to_owned();
    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&app_source, nth_offset(&app_source, "run()", 1)),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(&mut service, tools_uri.clone(), open_tools_source.clone()).await;
    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&app_source, nth_offset(&app_source, "run()", 1)),
    )
    .await
    .expect("dependency trait method call should use open workspace impl methods");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("single open-doc dependency trait impl should resolve to one location")
    };
    assert_eq!(uri, tools_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_tools_source,
            nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_source_for_dependency_trait_method_call()
{
    let temp =
        TempDir::new("ql-lsp-implementation-request-broken-open-dependency-trait-method-call");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
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
    let tools_path = temp.write(
        "workspace/packages/tools/src/lib.ql",
        r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn walk(self) -> Int {
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
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
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
    assert!(
        ql_analysis::analyze_source(&open_tools_source).is_err(),
        "open workspace impl source should stay broken for this regression",
    );

    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&app_source, nth_offset(&app_source, "run()", 1)),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(&mut service, tools_uri.clone(), open_tools_source.clone()).await;
    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&app_source, nth_offset(&app_source, "run()", 1)),
    )
    .await
    .expect("broken open dependency trait call should use open workspace impl methods");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("single broken open dependency trait impl should resolve to one location")
    };
    assert_eq!(uri, tools_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_tools_source,
            nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_open_workspace_source_for_broken_current_dependency_trait_method_call(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-current-dependency-trait-method-call",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run(
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
    let tools_path = temp.write(
        "workspace/packages/tools/src/lib.ql",
        r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn walk(self) -> Int {
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
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
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

    let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#
    .to_owned();
    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    assert!(
        ql_analysis::analyze_source(&app_source).is_err(),
        "current source should stay broken for this regression",
    );
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(&app_source, nth_offset(&app_source, "run(", 1)),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(&mut service, tools_uri.clone(), open_tools_source.clone()).await;
    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&app_source, nth_offset(&app_source, "run(", 1)),
    )
    .await
    .expect("broken dependency trait call should use open workspace impl methods");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("single broken dependency trait impl should resolve to one location")
    };
    assert_eq!(uri, tools_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_tools_source,
            nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_type_in_open_consumer() {
    let temp = TempDir::new("ql-lsp-implementation-request-open-same-named-local-dependency");
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
    let alpha_source_path = temp.write(
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

pub fn task() -> Int {
    return 0
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
    )
    .await
    .expect("matching open dependency implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching open dependency implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_task_source,
            nth_offset(&open_task_source, "extend Cfg", 1)
        ),
    );
    assert_ne!(
        uri.to_file_path()
            .expect("implementation URI should convert to file path")
            .canonicalize()
            .expect("implementation path should canonicalize"),
        alpha_source_path
            .canonicalize()
            .expect("alpha source path should canonicalize"),
        "implementation should come from the open consumer, not dependency source",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_trait_surface_in_open_consumer(
) {
    let temp =
        TempDir::new("ql-lsp-implementation-request-open-same-named-dependency-trait-surface");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Runner

pub fn main(runner: Runner) -> Int {
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
    let alpha_source_path = temp.write(
        "workspace/vendor/alpha/src/lib.ql",
        r#"
package demo.shared.alpha

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as OtherRunner

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl OtherRunner for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn task() -> Int {
    return 0
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset(&source, "Runner", 2)),
    )
    .await
    .expect("matching open trait surface implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching open trait surface implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(&open_task_source, nth_offset(&open_task_source, "impl Runner", 1)),
    );
    assert_ne!(
        uri.to_file_path()
            .expect("implementation URI should convert to file path")
            .canonicalize()
            .expect("implementation path should canonicalize"),
        alpha_source_path
            .canonicalize()
            .expect("alpha source path should canonicalize"),
        "trait surface implementation should come from the open consumer, not dependency source",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_trait_method_in_open_consumer(
) {
    let temp =
        TempDir::new("ql-lsp-implementation-request-open-same-named-dependency-trait-method");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
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
    let alpha_source_path = temp.write(
        "workspace/vendor/alpha/src/lib.ql",
        r#"
package demo.shared.alpha

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as OtherRunner

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl OtherRunner for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn task() -> Int {
    return 0
}
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset_in_context(&source, "run", "runner.run()", 1)),
    )
    .await
    .expect("matching open trait method implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching open trait method implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_task_source,
            nth_offset_in_context(&open_task_source, "run", "fn run(self) -> Int", 1),
        ),
    );
    assert_ne!(
        uri.to_file_path()
            .expect("implementation URI should convert to file path")
            .canonicalize()
            .expect("implementation path should canonicalize"),
        alpha_source_path
            .canonicalize()
            .expect("alpha source path should canonicalize"),
        "implementation should come from the open consumer, not dependency source",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_member_types_in_open_source(
) {
    let temp =
        TempDir::new("ql-lsp-implementation-request-open-same-named-dependency-member-types");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}

pub fn beta_extra() -> Bool {
    let next = other()
    return next.extra.flag
}

pub fn beta_pulse() -> Bool {
    let next = other()
    return next.pulse().flag
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

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
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

impl Counter {
    pub fn ping(self) -> Int
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

impl Counter {
    pub fn pulse(self) -> Extra
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

    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1),
        )
        .await
        .unwrap_or_else(|| panic!("same-named dependency implementation should exist for {needle}"));
        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!(
                "same-named dependency implementation should resolve to impl block locations: {implementation:?}"
            )
        };
        assert_eq!(locations.len(), 2);
        assert!(
            locations.iter().all(|location| location.uri == alpha_uri),
            "implementation should stay in the matching open alpha dependency source for {needle}",
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
                "implementation should include alpha {marker} for {needle}",
            );
        }
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
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
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
async fn implementation_request_prefers_matching_same_named_dependency_trait_surface_in_broken_open_consumer(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-open-same-named-dependency-trait-surface",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Runner

pub fn main(runner: Runner) -> Int {
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

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as OtherRunner

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl OtherRunner for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn broken() -> Int {
    return AlphaWorker {
"#
    .to_owned();
    let source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&source, nth_offset(&source, "Runner", 2)),
    )
    .await
    .expect("matching broken open trait surface implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken open trait surface implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(&open_task_source, nth_offset(&open_task_source, "impl Runner", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_trait_surface_in_broken_current_source(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-current-same-named-dependency-trait-surface",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as Peer

pub fn main(runner: Runner, peer: Peer) -> Int {
    return peer.
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

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as Peer

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl Peer for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn task() -> Int {
    return 0
}
"#
    .to_owned();
    let broken_source = fs::read_to_string(&app_path).expect("app source should read");
    assert!(
        ql_analysis::analyze_source(&broken_source).is_err(),
        "current source should stay broken for this regression",
    );
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), broken_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&broken_source, nth_offset(&broken_source, "Runner", 3)),
    )
    .await
    .expect("matching broken current trait surface implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken current trait surface implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(&open_task_source, nth_offset(&open_task_source, "impl Runner", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_type_in_broken_current_source(
) {
    let temp =
        TempDir::new("ql-lsp-implementation-request-broken-current-same-named-local-dependency");
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Config as Cfg
use demo.shared.beta.Config as Peer

pub fn main(current: Cfg, peer: Peer) -> Cfg {
    return Cfg {
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
use demo.shared.beta.Config as Peer

extend Cfg {
    fn alpha(self) -> Int {
        return 1
    }
}

extend Peer {
    fn beta(self) -> Bool {
        return true
    }
}

pub fn task() -> Int {
    return 0
}
"#
    .to_owned();
    let broken_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), broken_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&broken_source, nth_offset(&broken_source, "Cfg", 2)),
    )
    .await
    .expect("matching broken current dependency implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken current dependency implementation should stay scalar")
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
async fn implementation_request_prefers_matching_same_named_dependency_trait_method_in_broken_current_source(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-current-same-named-dependency-trait-method",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as Peer

pub fn main(runner: Runner, peer: Peer) -> Int {
    return runner.run(
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

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as Peer

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl Peer for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn task() -> Int {
    return 0
}
"#
    .to_owned();
    let broken_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, task_uri.clone(), open_task_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), broken_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        app_uri,
        offset_to_position(&broken_source, nth_offset(&broken_source, "run(", 1)),
    )
    .await
    .expect("matching broken current trait method implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken current trait method implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_task_source,
            nth_offset_in_context(&open_task_source, "run", "fn run(self) -> Int", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_trait_method_in_broken_open_consumer(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-open-same-named-dependency-trait-method",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
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

pub trait Runner {
    fn run(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/src/lib.ql",
        r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
    );

    let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as OtherRunner

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl OtherRunner for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn broken() -> Int {
    return AlphaWorker {
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
        offset_to_position(&source, nth_offset(&source, "run()", 1)),
    )
    .await
    .expect("matching broken open trait method implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("matching broken open trait method implementation should stay scalar")
    };
    assert_eq!(uri, task_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_task_source,
            nth_offset_in_context(&open_task_source, "run", "fn run(self) -> Int", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_member_types_in_broken_current_source(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-current-same-named-dependency-member-types",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}

pub fn beta_pulse() -> Bool {
    let next = other()
    return next.pulse(
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

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
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

impl Counter {
    pub fn pulse(self) -> Extra
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
    let broken_source = fs::read_to_string(&app_path).expect("app source should read");
    assert!(
        ql_analysis::analyze_source(&broken_source).is_err(),
        "current source should stay broken for this regression",
    );
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
    did_open_via_request(&mut service, app_uri.clone(), broken_source.clone()).await;

    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&broken_source, nth_offset(&broken_source, needle, occurrence) + 1),
        )
        .await
        .unwrap_or_else(|| {
            panic!(
                "broken-current dependency member type implementation should exist for {needle}"
            )
        });
        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!(
                "broken-current dependency member type implementation should resolve to impl block locations: {implementation:?}"
            )
        };
        assert_eq!(locations.len(), 2);
        assert!(
            locations.iter().all(|location| location.uri == alpha_uri),
            "implementation should stay in the matching alpha dependency source for {needle}",
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
                "implementation should include alpha {marker} for {needle}",
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_matching_same_named_dependency_member_types_in_broken_open_source(
) {
    let temp = TempDir::new(
        "ql-lsp-implementation-request-broken-open-same-named-dependency-member-types",
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}

pub fn beta_extra() -> Bool {
    let next = other()
    return next.extra.flag
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

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
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

impl Counter {
    pub fn ping(self) -> Int
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

impl Counter {
    pub fn pulse(self) -> Extra
}

pub fn build() -> Counter
"#,
    );

    let broken_open_alpha_source = r#"
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
    return Counter { value: 1, extra: Extra { id: 2 }
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
                text: broken_open_alpha_source.clone(),
            },
        })
        .await;
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, app_uri.clone(), source.clone()).await;

    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1),
        )
        .await
        .unwrap_or_else(|| {
            panic!(
                "broken-open dependency member type implementation should exist for {needle}"
            )
        });
        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!(
                "broken-open dependency member type implementation should resolve to impl block locations: {implementation:?}"
            )
        };
        assert_eq!(locations.len(), 2);
        assert!(
            locations.iter().all(|location| location.uri == alpha_uri),
            "implementation should stay in the matching alpha dependency source for {needle}",
        );
        for marker in ["impl Extra", "extend Extra"] {
            assert!(
                locations.iter().any(|location| {
                    location.range.start
                        == offset_to_position(
                            &broken_open_alpha_source,
                            nth_offset(&broken_open_alpha_source, marker, 1),
                        )
                }),
                "implementation should include alpha {marker} for {needle}",
            );
        }
    }
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
    return current.extra.id + current.pulse().id + next.pulse().flag
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

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
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

impl Counter {
    pub fn pulse(self) -> Extra
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

    for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
        let implementation = goto_implementation_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1),
        )
        .await
        .unwrap_or_else(|| {
            panic!("broken-source dependency member type implementation should exist for {needle}")
        });
        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!(
                "broken-source dependency member type implementation should resolve to impl block locations: {implementation:?}"
            )
        };
        assert_eq!(locations.len(), 2);
        assert!(
            locations.iter().all(|location| location.uri == alpha_uri),
            "implementation should stay in the matching alpha dependency source for {needle}",
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
                "implementation should include alpha {marker} for {needle}",
            );
        }
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
async fn implementation_request_uses_broken_open_workspace_source_for_workspace_type_import() {
    let fixture = setup_workspace_type_import_fixture(
        "ql-lsp-implementation-request-broken-open-workspace-type-import",
        r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
    );
    let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

pub trait Runner {
    fn run(self) -> Int
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

impl Runner for Config {
    fn run(self) -> Int {
        return self.value
    }
}

pub fn broken() -> Int {
    return Config {
"#
    .to_owned();
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        fixture.app_source.clone(),
    )
    .await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        fixture.app_uri.clone(),
        offset_to_position(
            &fixture.app_source,
            nth_offset(&fixture.app_source, "Config", 2),
        ),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        open_core_source.clone(),
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
    .expect("workspace type import implementation should use broken open workspace source");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken open workspace type import should resolve to many implementations")
    };
    assert_eq!(locations.len(), 3);
    assert!(
        locations
            .iter()
            .all(|location| location.uri == fixture.core_uri),
        "all broken open workspace type implementations should stay in the open source",
    );
    for marker in ["impl Config", "extend Config", "impl Runner for Config"] {
        assert!(
            locations.iter().any(|location| {
                location.range.start
                    == offset_to_position(
                        &open_core_source,
                        nth_offset(&open_core_source, marker, 1),
                    )
            }),
            "broken open workspace type implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_source_for_broken_current_workspace_type_import() {
    let fixture = setup_workspace_type_import_fixture(
        "ql-lsp-implementation-request-broken-current-workspace-type-import",
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
    let broken_app_source = r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return Config {
"#
    .to_owned();
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
        broken_app_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.app_uri.clone(),
        offset_to_position(
            &broken_app_source,
            nth_offset(&broken_app_source, "Config", 2),
        ),
    )
    .await
    .expect("broken current workspace type import implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken current workspace type import should resolve to many implementations")
    };
    assert_eq!(locations.len(), 2);
    assert!(
        locations
            .iter()
            .all(|location| location.uri == fixture.core_uri),
        "all broken current workspace type implementations should point at workspace source",
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
            "broken current workspace type implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_source_for_broken_current_workspace_trait_import() {
    let fixture = setup_workspace_root_runner_fixture(
        "ql-lsp-implementation-request-broken-current-workspace-trait-import",
    );
    let broken_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
    .to_owned();
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        broken_app_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.app_uri.clone(),
        offset_to_position(
            &broken_app_source,
            nth_offset(&broken_app_source, "Runner", 2),
        ),
    )
    .await
    .expect("broken current workspace trait import implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken current workspace trait import should resolve to many implementations")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source, marker) in [
        (
            fixture.app_uri.clone(),
            broken_app_source.as_str(),
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
            "broken current workspace trait implementations should include {marker}",
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
async fn implementation_request_returns_scalar_for_workspace_root_method_call() {
    let temp = TempDir::new("ql-lsp-implementation-request-workspace-root-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.get()
}
"#,
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.get()
}
"#,
    );
    let jobs_path = temp.write(
        "workspace/packages/jobs/src/lib.ql",
        r#"
package demo.jobs

use demo.core.Config

pub fn run(config: Config) -> Int {
    return config.get()
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/jobs"]
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
        "workspace/packages/jobs/qlang.toml",
        r#"
[package]
name = "jobs"

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

impl Config {
    pub fn get(self) -> Int
}
"#,
    );

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
    let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;
    did_open_via_request(&mut service, app_uri, app_source).await;
    did_open_via_request(&mut service, jobs_uri, jobs_source).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(&core_source, nth_offset(&core_source, "get()", 1)),
    )
    .await
    .expect("workspace root method implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("workspace root concrete method call should resolve to one implementation")
    };
    assert_eq!(uri, core_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "get", "pub fn get(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_none_for_workspace_root_method_definition_site() {
    let temp = TempDir::new("qlsp-implementation-request-workspace-root-method-definition-site");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.get()
}
"#,
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.get()
}
"#,
    );
    let jobs_path = temp.write(
        "workspace/packages/jobs/src/lib.ql",
        r#"
package demo.jobs

use demo.core.Config

pub fn run(config: Config) -> Int {
    return config.get()
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/jobs"]
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
        "workspace/packages/jobs/qlang.toml",
        r#"
[package]
name = "jobs"

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

impl Config {
    pub fn get(self) -> Int
}
"#,
    );

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
    let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;
    did_open_via_request(&mut service, app_uri, app_source).await;
    did_open_via_request(&mut service, jobs_uri, jobs_source).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri,
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "get", "pub fn get(self)", 1),
        ),
    )
    .await;
    assert_eq!(implementation, None);
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_source_for_root_method_call_with_broken_open_consumers() {
    let temp = TempDir::new("qlsp-implementation-request-broken-open-root-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return 0
}
"#,
    );
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

impl Config {
    pub fn pulse(self) -> Int
}
"#,
    );

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let open_app_source = r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.pulse(
"#
    .to_owned();

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;
    did_open_via_request(&mut service, app_uri, open_app_source).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(&core_source, nth_offset(&core_source, "pulse()", 1)),
    )
    .await
    .expect("workspace root method implementation should use broken open consumers");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("broken open workspace root method call should resolve to one implementation")
    };
    assert_eq!(uri, core_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "pulse", "pub fn pulse(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_none_for_root_method_definition_site_with_broken_open_consumers()
{
    let temp =
        TempDir::new("qlsp-implementation-request-broken-open-root-method-definition-site");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
    );
    let app_path = temp.write(
        "workspace/packages/app/src/main.ql",
        r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return 0
}
"#,
    );
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

impl Config {
    pub fn pulse(self) -> Int
}
"#,
    );

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let open_app_source = r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.pulse(
"#
    .to_owned();

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;
    did_open_via_request(&mut service, app_uri, open_app_source).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri,
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "pulse", "pub fn pulse(self)", 1),
        ),
    )
    .await;
    assert_eq!(implementation, None);
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_local_fallback_for_root_method_call_in_broken_current_source() {
    let temp = TempDir::new("qlsp-implementation-request-broken-current-root-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core"]
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

impl Config {
    pub fn pulse(self) -> Int
}
"#,
    );

    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse(
}
"#
    .to_owned();
    assert!(
        ql_analysis::analyze_source(&open_core_source).is_err(),
        "current root source should stay broken for this regression",
    );

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), open_core_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(&open_core_source, nth_offset(&open_core_source, "pulse", 2)),
    )
    .await
    .expect("broken current root method call should resolve with a local fallback");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("broken current workspace root method call should resolve to one implementation")
    };
    assert_eq!(uri, core_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_core_source,
            nth_offset_in_context(&open_core_source, "pulse", "pub fn pulse(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_none_for_root_method_definition_site_in_broken_current_source()
{
    let temp =
        TempDir::new("qlsp-implementation-request-broken-current-root-method-definition-site");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core"]
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

impl Config {
    pub fn pulse(self) -> Int
}
"#,
    );

    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse(
}
"#
    .to_owned();
    assert!(
        ql_analysis::analyze_source(&open_core_source).is_err(),
        "current root source should stay broken for this regression",
    );

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), open_core_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri,
        offset_to_position(
            &open_core_source,
            nth_offset_in_context(&open_core_source, "pulse", "pub fn pulse(self)", 1),
        ),
    )
    .await;
    assert_eq!(implementation, None);
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_none_for_ambiguous_root_method_call_in_broken_current_source()
{
    let temp = TempDir::new("qlsp-implementation-request-broken-current-root-method-ambiguous");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub struct Config {
    value: Int,
}

pub struct Other {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

impl Other {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core"]
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

pub struct Other {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int
}

impl Other {
    pub fn pulse(self) -> Int
}
"#,
    );

    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

pub struct Other {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

impl Other {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse(
}
"#
    .to_owned();
    assert!(
        ql_analysis::analyze_source(&open_core_source).is_err(),
        "current root source should stay broken for this regression",
    );

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri, open_core_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        Url::from_file_path(&core_path).expect("core path should convert to URI"),
        offset_to_position(&open_core_source, nth_offset(&open_core_source, "pulse", 3)),
    )
    .await;
    assert!(
        implementation.is_none(),
        "ambiguous broken current method calls should not guess an implementation",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_scalar_for_workspace_root_concrete_trait_method_call() {
    let temp =
        TempDir::new("ql-lsp-implementation-request-workspace-root-concrete-trait-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn call(worker: AppWorker) -> Int {
    return worker.run()
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core"]
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

pub struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int
}
"#,
    );

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "worker.run()", 1),
        ),
    )
    .await
    .expect("workspace root concrete trait method implementation should exist");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("workspace root concrete trait method call should resolve to one implementation")
    };
    assert_eq!(uri, core_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "fn run(self)", 2),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_returns_array_for_workspace_root_trait_method_call() {
    let temp = TempDir::new("ql-lsp-implementation-request-workspace-root-trait-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run()
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

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let tools_source = fs::read_to_string(&tools_path).expect("tools source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;
    did_open_via_request(&mut service, tools_uri.clone(), tools_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "runner.run()", 1),
        ),
    )
    .await
    .expect("workspace root trait method call implementation should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("workspace root trait method call should resolve to many implementations")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source) in [
        (app_uri.clone(), app_source.as_str()),
        (tools_uri.clone(), tools_source.as_str()),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(
                            source,
                            nth_offset_in_context(source, "run", "fn run(self)", 1),
                        )
            }),
            "workspace root trait method call should include {uri}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_prefers_open_workspace_source_for_workspace_root_trait_method_call() {
    let temp =
        TempDir::new("ql-lsp-implementation-request-open-workspace-root-trait-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run()
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
    fn walk(self) -> Int {
        return 2
    }
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core", "packages/tools"]
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
        "workspace/packages/tools/qlang.toml",
        r#"
[package]
name = "tools"

[references]
packages = ["../core"]
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

    let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#
    .to_owned();
    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "runner.run()", 1),
        ),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(&mut service, tools_uri.clone(), open_tools_source.clone()).await;
    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "runner.run()", 1),
        ),
    )
    .await
    .expect("workspace root trait method call should use open workspace impl methods");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("single open-doc root trait method call should resolve to one implementation")
    };
    assert_eq!(uri, tools_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_tools_source,
            nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_source_for_workspace_root_trait_method_call(
) {
    let temp =
        TempDir::new("ql-lsp-implementation-request-broken-open-root-trait-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run()
}
"#,
    );
    let tools_path = temp.write(
        "workspace/packages/tools/src/lib.ql",
        r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core", "packages/tools"]
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
        "workspace/packages/tools/qlang.toml",
        r#"
[package]
name = "tools"

[references]
packages = ["../core"]
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
    assert!(
        ql_analysis::analyze_source(&open_tools_source).is_err(),
        "open workspace impl source should stay broken for this regression",
    );

    let core_source = fs::read_to_string(&core_path).expect("core source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), core_source.clone()).await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "runner.run()", 1),
        ),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(&mut service, tools_uri.clone(), open_tools_source.clone()).await;
    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &core_source,
            nth_offset_in_context(&core_source, "run", "runner.run()", 1),
        ),
    )
    .await
    .expect("broken open workspace source should provide root trait method call implementation");
    let GotoImplementationResponse::Scalar(Location { uri, range }) = implementation else {
        panic!("single broken open root trait method call should resolve to one implementation")
    };
    assert_eq!(uri, tools_uri);
    assert_eq!(
        range.start,
        offset_to_position(
            &open_tools_source,
            nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_impls_for_broken_current_root_trait_method_call() {
    let temp =
        TempDir::new("ql-lsp-implementation-request-broken-current-root-trait-method-call");
    let core_path = temp.write(
        "workspace/packages/core/src/lib.ql",
        r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run(
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

    let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run(
}
"#
    .to_owned();
    assert!(
        ql_analysis::analyze_source(&open_core_source).is_err(),
        "current root source should stay broken for this regression",
    );

    let app_source = fs::read_to_string(&app_path).expect("app source should read");
    let tools_source = fs::read_to_string(&tools_path).expect("tools source should read");
    let core_uri = Url::from_file_path(&core_path).expect("core path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let tools_uri = Url::from_file_path(&tools_path).expect("tools path should convert to URI");

    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(&mut service, core_uri.clone(), open_core_source.clone()).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.clone()).await;
    did_open_via_request(&mut service, tools_uri.clone(), tools_source.clone()).await;

    let implementation = goto_implementation_via_request(
        &mut service,
        core_uri.clone(),
        offset_to_position(
            &open_core_source,
            nth_offset_in_context(&open_core_source, "run", "runner.run(", 1),
        ),
    )
    .await
    .expect("broken current root trait method implementations should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken current root trait method call should resolve to many implementations")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source) in [
        (app_uri.clone(), app_source.as_str()),
        (tools_uri.clone(), tools_source.as_str()),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(
                            source,
                            nth_offset_in_context(source, "run", "fn run(self)", 1),
                        )
            }),
            "broken current root trait method implementations should include {uri}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_impls_for_broken_current_root_trait_surface() {
    let fixture = setup_workspace_root_trait_single_consumer_fixture(
        "ql-lsp-implementation-request-broken-current-root-trait",
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
    let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn broken() -> Int {
    return 0
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
        fixture.core_uri.clone(),
        offset_to_position(
            &open_core_source,
            nth_offset(&open_core_source, "Runner", 1),
        ),
    )
    .await
    .expect("broken current root trait implementation should exist");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("single broken current root trait surface should resolve to one implementation")
    };
    assert_eq!(location.uri, fixture.app_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(
            &fixture.app_source,
            nth_offset(&fixture.app_source, "impl Runner for AppWorker", 1)
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_source_for_root_trait_surface() {
    let fixture = setup_workspace_root_trait_single_consumer_fixture(
        "ql-lsp-implementation-request-broken-open-root-trait",
        r#"
package demo.app

pub fn main() -> Int {
    return 0
}
"#,
    );
    let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
    .to_owned();
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        fixture.core_uri.clone(),
        offset_to_position(
            &fixture.core_source,
            nth_offset(&fixture.core_source, "Runner", 1),
        ),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        open_app_source.clone(),
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
    .expect("broken open workspace source should provide root trait implementation");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("single broken open root trait surface should resolve to one implementation")
    };
    assert_eq!(location.uri, fixture.app_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(
            &open_app_source,
            nth_offset(&open_app_source, "impl Runner for AppWorker", 1)
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_impls_for_broken_current_root_trait_surface_aggregation(
) {
    let fixture = setup_workspace_root_runner_fixture(
        "ql-lsp-implementation-request-broken-current-root-trait-aggregation",
    );
    let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn broken() -> Int {
    return 0
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
            &open_core_source,
            nth_offset(&open_core_source, "Runner", 1),
        ),
    )
    .await
    .expect("broken current root trait implementations should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken current root trait surface should aggregate many implementation blocks")
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
            "broken current root trait implementations should include {marker}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_sources_for_root_trait_surface_aggregation(
) {
    let fixture = setup_workspace_root_runner_fixture(
        "ql-lsp-implementation-request-broken-open-root-trait-aggregation",
    );
    let open_app_source = r#"
package demo.app

use demo.core.Runner

pub fn tag() -> Int {
    return 10
}

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
    .to_owned();
    let open_tools_source = r#"
package demo.tools

use demo.core.Runner

pub fn tag() -> Int {
    return 20
}

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
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        open_app_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.tools_uri.clone(),
        open_tools_source.clone(),
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
    .expect("broken open workspace sources should provide root trait implementations");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken open root trait surface should aggregate many implementation blocks")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source, marker) in [
        (
            fixture.app_uri.clone(),
            open_app_source.as_str(),
            "impl Runner for AppWorker",
        ),
        (
            fixture.tools_uri.clone(),
            open_tools_source.as_str(),
            "impl Runner for ToolWorker",
        ),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, marker, 1))
            }),
            "broken open root trait implementations should include {marker}",
        );
    }
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
async fn implementation_request_prefers_open_workspace_source_for_root_trait_method_definition()
{
    let fixture = setup_workspace_root_trait_single_consumer_fixture(
        "ql-lsp-implementation-request-open-root-trait-method-definition",
        r#"
package demo.app

struct AppWorker {}
"#,
    );
    let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#
    .to_owned();
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        fixture.core_uri.clone(),
        offset_to_position(
            &fixture.core_source,
            nth_offset(&fixture.core_source, "run", 1),
        ),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        open_app_source.clone(),
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
    .expect("open workspace source should provide root trait method implementation");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("single open root trait method should resolve to one implementation")
    };
    assert_eq!(location.uri, fixture.app_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(&open_app_source, nth_offset(&open_app_source, "run", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_source_for_root_trait_method_definition()
{
    let fixture = setup_workspace_root_trait_single_consumer_fixture(
        "ql-lsp-implementation-request-broken-open-root-trait-method-definition",
        r#"
package demo.app

struct AppWorker {}
"#,
    );
    let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
    .to_owned();
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;

    let disk_only = goto_implementation_via_request(
        &mut service,
        fixture.core_uri.clone(),
        offset_to_position(
            &fixture.core_source,
            nth_offset(&fixture.core_source, "run", 1),
        ),
    )
    .await;
    assert_eq!(disk_only, None);

    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        open_app_source.clone(),
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
    .expect("broken open workspace source should provide root trait method implementation");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("single broken open root trait method should resolve to one implementation")
    };
    assert_eq!(location.uri, fixture.app_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(&open_app_source, nth_offset(&open_app_source, "run", 1)),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_impls_for_broken_current_root_trait_method_definition(
) {
    let fixture = setup_workspace_root_trait_single_consumer_fixture(
        "ql-lsp-implementation-request-broken-current-root-trait-method-definition",
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
    let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn broken() -> Int {
    return 0
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
        fixture.core_uri.clone(),
        offset_to_position(&open_core_source, nth_offset(&open_core_source, "run", 1)),
    )
    .await
    .expect("broken current root trait method implementation should exist");
    let GotoImplementationResponse::Scalar(location) = implementation else {
        panic!("single broken current root trait method should resolve to one implementation")
    };
    assert_eq!(location.uri, fixture.app_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(
            &fixture.app_source,
            nth_offset(&fixture.app_source, "run", 1)
        ),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_broken_open_workspace_sources_for_root_trait_method_definition_aggregation(
) {
    let fixture = setup_workspace_root_runner_fixture(
        "ql-lsp-implementation-request-broken-open-root-trait-method-definition-aggregation",
    );
    let open_app_source = r#"
package demo.app

use demo.core.Runner

pub fn tag() -> Int {
    return 10
}

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
    .to_owned();
    let open_tools_source = r#"
package demo.tools

use demo.core.Runner

pub fn tag() -> Int {
    return 20
}

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
    did_open_via_request(
        &mut service,
        fixture.core_uri.clone(),
        fixture.core_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.app_uri.clone(),
        open_app_source.clone(),
    )
    .await;
    did_open_via_request(
        &mut service,
        fixture.tools_uri.clone(),
        open_tools_source.clone(),
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
    .expect("broken open workspace sources should provide root trait method implementations");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken open root trait method definition should aggregate many implementations")
    };
    assert_eq!(locations.len(), 2);
    for (uri, source) in [
        (fixture.app_uri.clone(), open_app_source.as_str()),
        (fixture.tools_uri.clone(), open_tools_source.as_str()),
    ] {
        assert!(
            locations.iter().any(|location| {
                location.uri == uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, "run", 1))
            }),
            "broken open root trait method implementations should include {uri}",
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn implementation_request_uses_workspace_impls_for_broken_current_root_trait_method_definition_aggregation(
) {
    let fixture = setup_workspace_root_runner_fixture(
        "ql-lsp-implementation-request-broken-current-root-trait-method-definition-aggregation",
    );
    let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn broken() -> Int {
    return 0
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
    did_open_via_request(
        &mut service,
        fixture.tools_uri.clone(),
        fixture.tools_source.clone(),
    )
    .await;

    let implementation = goto_implementation_via_request(
        &mut service,
        fixture.core_uri.clone(),
        offset_to_position(&open_core_source, nth_offset(&open_core_source, "run", 1)),
    )
    .await
    .expect("broken current root trait method implementations should exist");
    let GotoImplementationResponse::Array(locations) = implementation else {
        panic!("broken current root trait method definition should aggregate many implementations")
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
            "broken current root trait method implementations should include {uri}",
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
