use ql_analysis::analyze_source as analyze_semantics;

use crate::analysis_commands::{
    render_mir_path, render_ownership_path, render_runtime_requirements,
};
use crate::cli_analysis::analyze_source;
use crate::tests::support::TestDir;

#[test]
fn analyze_source_reports_semantic_errors() {
    let diagnostics = analyze_source(
        r#"
struct User {}
fn User() {}
"#,
    )
    .expect_err("source should have semantic diagnostics");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate top-level definition `User`")
    );
}

#[test]
fn analyze_source_reports_resolution_errors() {
    let diagnostics = analyze_source(
        r#"
fn main() -> Int {
    self
}
"#,
    )
    .expect_err("source should have resolver diagnostics");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "invalid use of `self` outside a method receiver scope"
    }));
}

#[test]
fn analyze_source_reports_type_errors() {
    let diagnostics = analyze_source(
        r#"
fn main() -> Int {
    return "oops"
}
"#,
    )
    .expect_err("source should have type diagnostics");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "return value has type mismatch: expected `Int`, found `String`"
    }));
}

#[test]
fn render_mir_path_succeeds_for_valid_sources() {
    let dir = TestDir::new("ql-cli-mir");
    dir.write(
        "sample.ql",
        r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
    );

    assert!(render_mir_path(&dir.path().join("sample.ql")).is_ok());
}

#[test]
fn render_ownership_path_surfaces_ownership_reports() {
    let dir = TestDir::new("ql-cli-ownership");
    dir.write(
        "sample.ql",
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#,
    );

    let result = render_ownership_path(&dir.path().join("sample.ql"));
    assert!(
        result.is_err(),
        "ownership diagnostics should fail the command"
    );
}

#[test]
fn render_runtime_requirements_reports_async_surface() {
    let analysis = analyze_semantics(
        r#"
async fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    let task = spawn helper()
    return await helper()
}

async fn helper() -> Int {
    return 1
}
"#,
    )
    .expect("source should analyze");

    let rendered = render_runtime_requirements(&analysis);
    assert!(rendered.contains("runtime requirement: async-function-bodies @"));
    assert!(rendered.contains("runtime requirement: async-iteration @"));
    assert!(rendered.contains("runtime requirement: task-spawn @"));
    assert!(rendered.contains("runtime requirement: task-await @"));
    assert!(rendered.contains("runtime hook: async-frame-alloc -> qlrt_async_frame_alloc"));
    assert!(rendered.contains("runtime hook: async-task-create -> qlrt_async_task_create"));
    assert!(rendered.contains("runtime hook: executor-spawn -> qlrt_executor_spawn"));
    assert!(rendered.contains("runtime hook: task-await -> qlrt_task_await"));
    assert!(rendered.contains("runtime hook: task-result-release -> qlrt_task_result_release"));
    assert!(rendered.contains("runtime hook: async-iter-next -> qlrt_async_iter_next"));
    assert!(rendered.contains(
        "runtime hook abi: async-frame-alloc ccc qlrt_async_frame_alloc(size: i64, align: i64) -> ptr"
    ));
    assert!(rendered.contains(
        "runtime hook abi: async-task-create ccc qlrt_async_task_create(entry_fn: ptr, frame: ptr) -> ptr"
    ));
    assert!(rendered.contains(
        "runtime hook abi: executor-spawn ccc qlrt_executor_spawn(executor: ptr, task: ptr) -> ptr"
    ));
    assert!(
        rendered.contains("runtime hook abi: task-await ccc qlrt_task_await(handle: ptr) -> ptr")
    );
    assert!(rendered.contains(
        "runtime hook abi: task-result-release ccc qlrt_task_result_release(result: ptr) -> void"
    ));
    assert!(rendered.contains(
        "runtime hook abi: async-iter-next ccc qlrt_async_iter_next(iterator: ptr) -> ptr"
    ));
}

#[test]
fn render_runtime_requirements_reports_none_for_sync_sources() {
    let analysis = analyze_semantics(
        r#"
fn main() -> Int {
    return 1
}
"#,
    )
    .expect("source should analyze");

    assert_eq!(
        render_runtime_requirements(&analysis),
        "runtime requirements: none\n"
    );
}
