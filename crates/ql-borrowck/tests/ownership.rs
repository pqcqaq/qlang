use ql_borrowck::{analyze_module as analyze_borrowck, render_result};
use ql_diagnostics::Diagnostic;
use ql_hir::lower_module as lower_hir;
use ql_mir::lower_module as lower_mir;
use ql_parser::parse_source;
use ql_resolve::resolve_module;
use ql_typeck::analyze_module as analyze_types;

fn diagnostic_messages(source: &str) -> Vec<String> {
    borrowck_diagnostics(source)
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn borrowck_diagnostics(source: &str) -> Vec<Diagnostic> {
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_hir(&ast);
    let resolution = resolve_module(&hir);
    let typeck = analyze_types(&hir, &resolution);
    let mir = lower_mir(&hir, &resolution);
    let borrowck = analyze_borrowck(&hir, &resolution, &typeck, &mir);

    borrowck.diagnostics().to_vec()
}

fn render_output(source: &str) -> String {
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_hir(&ast);
    let resolution = resolve_module(&hir);
    let typeck = analyze_types(&hir, &resolution);
    let mir = lower_mir(&hir, &resolution);
    let borrowck = analyze_borrowck(&hir, &resolution, &typeck, &mir);
    render_result(&borrowck, &mir)
}

#[test]
fn reports_use_after_move_from_move_self_method() {
    let diagnostics = diagnostic_messages(
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

    assert!(diagnostics.contains(&"local `user` was used after move".to_string()));
}

#[test]
fn reports_maybe_moved_after_branch_join() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main(flag: Bool) -> String {
    let user = User { name: "ql" }
    if flag {
        user.into_json()
    } else {
        ""
    }
    return user.name
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `user` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reassigning_a_local_makes_it_available_again() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn fresh_user() -> User {
    return User { name: "new" }
}

fn main() -> String {
    let user = User { name: "old" }
    user.into_json();
    user = fresh_user();
    return user.name
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn move_receivers_consume_after_argument_evaluation() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn rename(move self, to: String) -> User {
        return User { name: to }
    }
}

fn main() -> User {
    let user = User { name: "ql" }
    return user.rename(user.name)
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn move_closure_captures_consume_direct_locals() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let value = 1
    let capture = move () => value
    return value
}
"#,
    );

    assert!(diagnostics.contains(&"local `value` was used after move".to_string()));
}

#[test]
fn reports_use_after_awaiting_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let first = await task
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_use_after_spawning_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let running = spawn task
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn allows_spawning_bound_helper_returned_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected helper-returned task handles to be spawnable after binding, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_spawning_helper_returned_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let task = schedule()
    let running = spawn task
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn allows_spawning_bound_zero_sized_helper_returned_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected zero-sized helper-returned task handles to be spawnable after binding, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_spawning_zero_sized_helper_returned_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    let task = schedule()
    let running = spawn task
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_maybe_moved_task_handle_after_branch_join() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main(flag: Bool) -> Int {
    let task = worker()
    if flag {
        await task
    } else {
        0
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reports_maybe_moved_zero_sized_task_handle_after_branch_join() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    let task = worker()
    if flag {
        await task
    } else {
        Wrap { values: [] }
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reports_maybe_moved_zero_sized_task_handle_after_branch_join_and_helper_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        forward(task)
    } else {
        task = fresh_worker()
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reassigning_a_task_handle_makes_it_available_again() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    var task = worker()
    let first = await task
    task = worker()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_passing_task_handle_to_helper() {
    let diagnostics = diagnostic_messages(
        r#"
fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let forwarded = forward(task)
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_use_after_passing_zero_sized_task_handle_to_helper() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let task = worker()
    let forwarded = forward(task)
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn allows_conditionally_returning_zero_sized_task_handles_from_helpers() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn choose(flag: Bool, first: Task[Wrap], second: Task[Wrap]) -> Task[Wrap] {
    if flag {
        return first
    }
    return second
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag, worker(), worker())
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_named_task_handle_helper_argument_as_move() {
    let diagnostics = diagnostic_messages(
        r#"
fn pair(value: Int, task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let forwarded = pair(value: 1, task: task)
    return await task
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_maybe_moved_after_conditionally_passing_task_handle_to_helper() {
    let diagnostics = diagnostic_messages(
        r#"
fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main(flag: Bool) -> Int {
    let task = worker()
    if flag {
        forward(task)
    } else {
        worker()
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reports_maybe_moved_after_conditionally_passing_zero_sized_task_handle_to_helper() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    let task = worker()
    if flag {
        forward(task)
    } else {
        worker()
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reassigning_a_helper_consumed_task_handle_makes_it_available_again() {
    let diagnostics = diagnostic_messages(
        r#"
fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    var task = worker()
    let forwarded = forward(task)
    task = worker()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reassigning_a_zero_sized_helper_consumed_task_handle_makes_it_available_again() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var task = worker()
    let forwarded = forward(task)
    task = worker()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn deferred_cleanup_reinitializes_zero_sized_task_handles_for_later_reads() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Int {
    var task = worker()
    defer await task
    defer {
        task = fresh_worker();
        ""
    }
    defer await task
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_deferred_cleanup_use_after_helper_consumes_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    defer await task
    defer forward(task)
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_deferred_cleanup_use_after_zero_sized_helper_consumes_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Int {
    let task = worker()
    defer await task
    defer forward(task)
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_deferred_cleanup_use_after_returning_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
fn sink(task: Task[Int]) -> Void {
    return
}

fn forward(task: Task[Int]) -> Task[Int] {
    defer sink(task)
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let forwarded = forward(worker())
    return await forwarded
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_deferred_cleanup_use_after_returning_zero_sized_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn sink(task: Task[Wrap]) -> Void {
    return
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    defer sink(task)
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let forwarded = forward(worker())
    return await forwarded
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn closure_captures_read_moved_direct_locals() {
    let diagnostics = diagnostic_messages(
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
    user.into_json();
    let inspect = () => user.name
    return ""
}
"#,
    );

    assert!(diagnostics.contains(&"local `user` was used after move".to_string()));
}

#[test]
fn move_closure_capture_diagnostics_anchor_to_the_captured_name() {
    let source = r#"
fn main() -> Int {
    let value = 1
    let capture = move () => value
    return value
}
"#;
    let diagnostics = borrowck_diagnostics(source);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message == "local `value` was used after move")
        .expect("expected move-closure capture diagnostic");
    let capture_span = diagnostic
        .labels
        .iter()
        .find(|label| label.message.as_deref() == Some("captured here by `move` closure"))
        .expect("expected move-closure capture label")
        .span;
    let expected_start = source
        .find("move () => value")
        .expect("closure expression should exist")
        + "move () => ".len();

    assert_eq!(capture_span.start, expected_start);
    assert_eq!(capture_span.len(), "value".len());
}

#[test]
fn readonly_and_mutable_receivers_do_not_count_as_move() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self) -> Int {
        return self.value
    }

    fn bump(var self) -> Int {
        self.value = self.value + 1
        return self.value
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    counter.bump()
    return counter.read()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn ambiguous_method_candidates_do_not_trigger_consumption() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

extend User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json();
    return user.name
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics for ambiguous method candidates, got {diagnostics:?}"
    );
}

#[test]
fn reports_deferred_cleanup_use_after_prior_cleanup_move() {
    let diagnostics = diagnostic_messages(
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
    defer user.name
    defer user.into_json()
    return ""
}
"#,
    );

    assert!(diagnostics.contains(&"local `user` was used after move".to_string()));
}

#[test]
fn reports_maybe_moved_from_conditional_cleanup() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main(flag: Bool) -> String {
    let user = User { name: "ql" }
    defer user.name
    defer if flag { user.into_json() } else { "" }
    return ""
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `user` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reports_maybe_moved_zero_sized_task_handle_from_conditional_cleanup() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { await task } else { await fresh_worker() }
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reports_maybe_moved_zero_sized_task_handle_from_conditional_helper_cleanup() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { forward(worker()) }
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn allows_conditional_cleanup_reinitializing_zero_sized_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer if flag { task = fresh_worker(); "" } else { "" }
    defer await task
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_maybe_moved_zero_sized_task_handle_from_conditional_cleanup_reinit_and_consume() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { task = fresh_worker(); task } else { forward(task) }
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `task` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reports_use_after_zero_sized_task_handle_from_conditional_cleanup_reinit_and_helper_consume() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

    async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { task = fresh_worker(); forward(task) } else { forward(task) }
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn reports_use_after_zero_sized_task_handle_from_conditional_cleanup_helper_consume_and_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { task = fresh_worker(); forward(task) }
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"local `task` was used after move".to_string()));
}

#[test]
fn allows_guarded_cleanup_reinitializing_dynamic_task_handle_literal_before_scope_exit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    defer if index == 0 { forward(tasks[index]) } else { forward(worker()) }
    if index != 0 {
        return await tasks[0]
    };
    tasks[0] = worker()
    return await worker()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded cleanup to reuse tasks[0] after literal reinit without maybe-moved diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn allows_guarded_cleanup_reinitializing_projected_dynamic_task_handle_literal_before_scope_exit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(slot: Slot) -> Wrap {
    var tasks = [worker(), worker()]
    defer if slot.value == 0 { forward(tasks[slot.value]) } else { forward(worker()) }
    if slot.value != 0 {
        return await tasks[0]
    };
    tasks[0] = worker()
    return await worker()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected guarded cleanup to reuse tasks[0] after literal reinit without maybe-moved diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn deferred_root_write_reinitializes_for_later_cleanup_reads() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn fresh_user() -> User {
    return User { name: "new" }
}

fn main() -> String {
    let user = User { name: "old" }
    defer user.name
    defer {
        user = fresh_user();
        ""
    }
    defer user.into_json()
    return ""
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn renders_block_state_facts_for_debugging() {
    let rendered = render_output(
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

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("bb0 in=["));
    assert!(rendered.contains("consume(move self into_json)"));
}

#[test]
fn renders_cleanup_effects_for_debugging() {
    let rendered = render_output(
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
    defer user.name
    defer user.into_json()
    return ""
}
"#,
    );

    assert!(rendered.contains("consume(move self into_json)"));
    assert!(rendered.contains("read @"));
}

#[test]
fn renders_zero_sized_task_cleanup_reinitialization_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Int {
    var task = worker()
    defer await task
    defer {
        task = fresh_worker();
        ""
    }
    defer await task
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert_eq!(
        rendered.matches("consume(await task handle)").count(),
        2,
        "expected both deferred await uses to be rendered, got {rendered}"
    );
}

#[test]
fn renders_zero_sized_task_conditional_cleanup_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { await task } else { await fresh_worker() }
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_zero_sized_task_conditional_helper_cleanup_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { forward(worker()) }
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
    assert!(rendered.contains("consume(call task handle argument)"));
}

#[test]
fn renders_zero_sized_task_conditional_helper_cleanup_maybe_moved_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { forward(worker()) }
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
    assert!(rendered.contains("consume(call task handle argument)"));
}

#[test]
fn renders_zero_sized_task_conditional_cleanup_helper_consume_and_reinit_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { task = fresh_worker(); forward(task) }
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
    assert!(rendered.contains("consume(call task handle argument)"));
}

#[test]
fn renders_zero_sized_task_conditional_cleanup_reinitialization_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer if flag { task = fresh_worker(); "" } else { "" }
    defer await task
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_zero_sized_task_conditional_cleanup_reinit_and_consume_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { task = fresh_worker(); task } else { forward(task) }
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
    assert!(rendered.contains("consume(call task handle argument)"));
}

#[test]
fn renders_zero_sized_task_conditional_cleanup_reinit_and_helper_consume_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { task = fresh_worker(); forward(task) } else { forward(task) }
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
    assert!(rendered.contains("consume(call task handle argument)"));
}

#[test]
fn renders_zero_sized_task_branch_join_and_helper_reinit_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        forward(task)
    } else {
        task = fresh_worker()
    }
    return await task
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(call task handle argument)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_zero_sized_task_branch_join_and_spawn_reinit_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        let running = spawn task;
        task = fresh_worker();
        await running
    } else {
        task = fresh_worker()
    }
    return await task
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(spawn task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_zero_sized_task_helper_conditionally_returning_spawned_task_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task;
        return await running
    }
    return await task
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
    );

    assert!(
        rendered.contains("ownership choose"),
        "rendered output:\n{rendered}"
    );
    assert!(
        rendered.contains("consume(await task handle)"),
        "rendered output:\n{rendered}"
    );
}

#[test]
fn renders_zero_sized_task_reverse_branch_helper_conditionally_returning_spawned_task_for_debugging()
 {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task;
    return await running
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
    );

    assert!(
        rendered.contains("ownership choose"),
        "rendered output:\n{rendered}"
    );
    assert!(
        rendered.contains("consume(await task handle)"),
        "rendered output:\n{rendered}"
    );
}

#[test]
fn renders_zero_sized_task_cleanup_helper_consumes_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Int {
    let task = worker()
    defer await task
    defer forward(task)
    return 0
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(await task handle)"));
    assert!(rendered.contains("consume(call task handle argument)"));
}

#[test]
fn renders_move_closure_capture_effects_for_debugging() {
    let rendered = render_output(
        r#"
fn main() -> Int {
    let value = 1
    let capture = move () => value
    return value
}
"#,
    );

    assert!(rendered.contains("consume(move closure capture)"));
}

#[test]
fn renders_async_task_handle_consumes_for_debugging() {
    let rendered = render_output(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(rendered.contains("consume(spawn task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn allows_awaiting_projected_task_handle_from_tuple() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = (worker(), worker())
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected task-handle await to be accepted, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_projected_task_handle_from_tuple() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = (worker(), worker())
    let running = spawn pair[0]
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected task-handle spawn to be accepted, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_sibling_projected_task_handles_from_tuple() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = (worker(), worker())
    let first = await pair[0]
    return await pair[1]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected sibling projected task-handle awaits to be accepted, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_sibling_projected_task_handles_from_fixed_arrays() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = [worker(), worker()]
    let first = await pair[0]
    return await pair[1]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected sibling projected fixed-array task-handle awaits to be accepted, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_reawaiting_same_projected_task_handle_from_tuple() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = (worker(), worker())
    let first = await pair[0]
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `pair` was used after move".to_string()),
        "expected re-awaiting the same projected task handle to report a move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_reawaiting_same_projected_task_handle_from_fixed_array() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = [worker(), worker()]
    let first = await pair[0]
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `pair` was used after move".to_string()),
        "expected re-awaiting the same fixed-array projected task handle to report a move, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_sibling_projected_task_handle_after_conditional_tuple_move() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main(flag: Bool) -> Int {
    let pair = (worker(), worker())
    if flag {
        let first = await pair[0]
    }
    return await pair[1]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional projected move on a sibling tuple element to be accepted, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_projected_task_handle_from_struct_field() {
    let diagnostics = diagnostic_messages(
        r#"
struct TaskPair {
    task: Task[Int],
    value: Int,
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = TaskPair { task: worker(), value: 1 }
    return await pair.task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected struct-field task-handle await to be accepted, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_sibling_projected_task_handles_from_struct_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = TaskPair { left: worker(), right: worker() }
    let first = await pair.left
    return await pair.right
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected sibling projected struct-field task handles to remain independently usable, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_awaiting_same_projected_task_handle_from_struct_field() {
    let diagnostics = diagnostic_messages(
        r#"
struct TaskPair {
    task: Task[Int],
    value: Int,
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = TaskPair { task: worker(), value: 1 }
    let first = await pair.task
    return await pair.task
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `pair` was used after move".to_string()),
        "expected projected struct-field task-handle await to consume the struct base local, got {diagnostics:?}"
    );
}

#[test]
fn allows_reading_sibling_value_field_after_awaiting_projected_task_handle() {
    let diagnostics = diagnostic_messages(
        r#"
struct TaskPair {
    task: Task[Int],
    value: Int,
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let pair = TaskPair { task: worker(), value: 7 }
    let first = await pair.task
    return pair.value
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected sibling non-task field reads to remain usable after consuming a projected task handle, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_projected_task_handle_from_tuple_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    var pair = (worker(), worker())
    let first = await pair[0]
    pair[0] = worker()
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected tuple projection reinitialization to restore the task handle path, got {diagnostics:?}"
    );
}

#[test]
fn allows_conditionally_reinitializing_projected_task_handle_before_branch_join() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main(flag: Bool) -> Int {
    var pair = (worker(), worker())
    if flag {
        let first = await pair[0]
        pair[0] = worker()
    }
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional tuple projection reinitialization to clear maybe-moved facts, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_projected_task_handle_from_struct_field_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    var pair = TaskPair { left: worker(), right: worker() }
    let first = await pair.left
    pair.left = worker()
    return await pair.left
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected struct-field projection reinitialization to restore the task handle path, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_projected_task_handle_from_fixed_array_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[0]
    tasks[0] = worker()
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected fixed-array projection reinitialization to restore the task handle path, got {diagnostics:?}"
    );
}

#[test]
fn allows_conditionally_reinitializing_projected_task_handle_from_fixed_array_before_branch_join() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional fixed-array projection reinitialization to clear maybe-moved facts, got {diagnostics:?}"
    );
}

#[test]
fn allows_dynamic_task_handle_array_index_assignment_without_prior_consumes() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    tasks[index] = worker()
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected dynamic task-handle array assignment without prior consumes to avoid borrowck regressions, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_dynamic_task_handle_array_index_without_consuming_sibling_task_field() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
    fallback: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let pending = Pending {
        tasks: [worker(), worker()],
        fallback: worker(),
    }
    let first = await pending.tasks[index]
    return await pending.fallback
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected dynamic task-handle await to stay path-sensitive for sibling task fields, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_dynamic_task_handle_array_index_without_consuming_sibling_task_field() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
    fallback: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let pending = Pending {
        tasks: [worker(), worker()],
        fallback: worker(),
    }
    let running = spawn pending.tasks[index]
    let first = await running
    return await pending.fallback
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected dynamic task-handle spawn to stay path-sensitive for sibling task fields, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_alias_sourced_composed_dynamic_task_handle_to_helper_without_consuming_sibling_task_field()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
    fallback: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main(row: Int) -> Wrap {
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(), worker()],
        fallback: worker(),
    }
    let alias = pending.tasks
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker()
    let forwarded = forward(alias[alias_slots[row]])
    return await pending.fallback
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected helper-consumed alias-sourced composed dynamic task-handle path to stay path-sensitive for sibling task fields, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_alias_sourced_composed_dynamic_task_handle_to_helper_without_consuming_sibling_task_field()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
    fallback: Task[Wrap],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn choose() -> Int {
    return INDEX
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row = choose()
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(), worker()],
        fallback: worker(),
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    return await pending.fallback
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded helper-consumed alias-sourced composed dynamic task-handle path to stay path-sensitive for sibling task fields, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_const_backed_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row = INDEX
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let alias = pending.tasks
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker()
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected const-backed alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row = INDEX
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row = INDEX
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let alias = root
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed double-root alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row = INDEX
    let slots = [row, row]
    let slot_root = slots
    let alias_slots = slot_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let alias = root
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed double-root double-source alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let alias_slots = slot_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let alias = root
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed double-root double-source row-alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let alias_slots = slot_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let alias = root
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed double-root double-source row-slot-alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let alias_slots = slot_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root double-source row-slot-alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let forwarded = forward(alias[alias_slots[row]])
    let second = await pending.tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let second = await tail_tasks[1]
    return await forwarded
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail-alias-sourced composed dynamic helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_passing_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_alias_task_handle_to_helper_and_consuming_sibling_array_element()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let second = await tail_tasks[1]
    return await running_task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail-alias-sourced forwarded-alias helper consume to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let queued = forwarded
    let second = await tail_tasks[1]
    let running = spawn queued
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queued_tasks = env.bundle.tasks
    let second = await env.tail
    let running = spawn queued_tasks[0]
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_alias_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queued_tasks = queue_root
    let second = await env.tail
    let running = spawn queued_tasks[0]
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-alias spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_chain_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let second = await env.tail
    let running = spawn queued_tasks[0]
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-chain spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_local_alias_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_alias = queued
    let second = await env.tail
    let running = spawn queued_alias
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-local-alias spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_local_chain_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_alias = queued
    let queued_final = queued_alias
    let second = await env.tail
    let running = spawn queued_final
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-local-chain spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_local_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_alias = queued
    let queued_final = queued_alias
    let queued_ready = forward(queued_final)
    let second = await env.tail
    let running = spawn queued_ready
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-local-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_local_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_alias = queued
    let queued_final = queued_alias
    let second = await env.tail
    let running = spawn forward(queued_final)
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-local-inline-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let second = await env.tail
    let running = spawn forward(env.bundle.tasks[0])
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-inline-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_slot_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_slot = Slot { value: INDEX }
    let bundle_slot_alias = bundle_slot
    let second = await env.tail
    let running = spawn forward(env.bundle.tasks[bundle_slot_alias.value])
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-slot-inline-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_tasks = env.bundle.tasks
    let bundled = bundle_tasks[0]
    let bundle_ready = forward(bundled)
    let second = await env.tail
    let running = spawn bundle_ready
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_alias_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_root = env.bundle.tasks
    let bundle_tasks = bundle_root
    let bundled = bundle_tasks[0]
    let bundle_ready = forward(bundled)
    let second = await env.tail
    let running = spawn bundle_ready
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-alias-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_chain_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_root = env.bundle.tasks
    let bundle_alias_root = bundle_root
    let bundle_tasks = bundle_alias_root
    let bundled = bundle_tasks[0]
    let bundle_ready = forward(bundled)
    let second = await env.tail
    let running = spawn bundle_ready
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-chain-forwarded spawn after forwarded alias to preserve sibling array element availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_tail_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let second = await env.bundle.tasks[0]
    let running = spawn forward(env.tail)
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail tail-inline-forwarded spawn after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_tail_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let tail = await forward(env.tail)
    let second = await env.bundle.tasks[0]
    let extra = await env.bundle.tasks[1]
    return tail
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail tail-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_slot_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_slot = Slot { value: INDEX }
    let bundle_slot_alias = bundle_slot
    let second = await forward(env.bundle.tasks[bundle_slot_alias.value])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-slot-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let second = await forward(env.bundle.tasks[0])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_tasks = env.bundle.tasks
    let bundled = bundle_tasks[0]
    let bundle_ready = forward(bundled)
    let second = await bundle_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_alias_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_root = env.bundle.tasks
    let bundle_tasks = bundle_root
    let bundled = bundle_tasks[0]
    let bundle_ready = forward(bundled)
    let second = await bundle_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-alias-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_chain_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_root = env.bundle.tasks
    let bundle_alias_root = bundle_root
    let bundle_tasks = bundle_alias_root
    let bundled = bundle_tasks[0]
    let bundle_ready = forward(bundled)
    let second = await bundle_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-chain-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_alias_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_root = env.bundle.tasks
    let bundle_tasks = bundle_root
    let second = await forward(bundle_tasks[0])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-alias-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_bundle_chain_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let bundle_root = env.bundle.tasks
    let bundle_alias_root = bundle_root
    let bundle_tasks = bundle_alias_root
    let second = await forward(bundle_tasks[0])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail bundle-chain-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_local_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_alias = queued
    let queued_final = queued_alias
    let second = await forward(queued_final)
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-local-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_local_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_alias = queued
    let queued_final = queued_alias
    let queued_ready = forward(queued_final)
    let second = await queued_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-local-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queued_tasks = env.bundle.tasks
    let second = await forward(queued_tasks[0])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queued_tasks = env.bundle.tasks
    let queued = queued_tasks[0]
    let queued_ready = forward(queued)
    let second = await queued_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_alias_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queued_tasks = queue_root
    let queued = queued_tasks[0]
    let queued_ready = forward(queued)
    let second = await queued_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-alias-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_chain_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let queued_ready = forward(queued)
    let second = await queued_ready
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-chain-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_alias_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queued_tasks = queue_root
    let second = await forward(queued_tasks[0])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-alias-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_queued_root_chain_inline_forwarded_task_handle_after_forwarded_alias()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Wrap],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker()
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker()],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let second = await forward(queued_tasks[0])
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guarded const-backed triple-root triple-source row-slot-tail queued-root-chain-inline-forwarded await after forwarded alias to preserve sibling task availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_maybe_moved_for_specific_array_element_after_dynamic_task_handle_consume() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let pending = Pending {
        tasks: [worker(), worker()],
    }
    let first = await pending.tasks[index]
    return await pending.tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"local `pending` may have been moved on another control-flow path".to_string()
        ),
        "expected dynamic task-handle consume to stay conservative for specific array elements, got {diagnostics:?}"
    );
}

#[test]
fn reports_maybe_moved_after_dynamic_task_handle_array_reinit_with_prior_consume() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[0]
    tasks[index] = worker()
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"local `tasks` may have been moved on another control-flow path".to_string()
        ),
        "expected dynamic task-handle array assignment to stay conservative after prior consume, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_projected_root_dynamic_task_handle_array_index_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let slot = Slot { value: index }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker()
    return await pending.tasks[slot.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected-root dynamic task-handle index reinitialization to restore that path, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_immutable_dynamic_task_handle_array_index_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[index]
    tasks[index] = worker()
    return await tasks[index]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected same immutable dynamic task-handle index reinitialization to restore that path, got {diagnostics:?}"
    );
}

#[test]
fn allows_conditionally_reinitializing_same_immutable_dynamic_task_handle_array_index_before_branch_join()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool, index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[index]
        tasks[index] = worker()
    }
    return await tasks[index]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional same immutable dynamic index reinitialization to clear maybe-moved facts, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_guard_refined_dynamic_task_handle_array_index_and_literal_reuse() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    if index == 0 {
        let first = await tasks[index]
        return await tasks[0]
    }
    return await tasks[1]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected guard-refined dynamic task-handle index reuse through tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_guard_refined_dynamic_task_handle_array_index_before_literal_reuse() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker()
    }
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guard-refined dynamic task-handle reinit through tasks[0] to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_immutable_dynamic_task_handle_array_index_without_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let first = await tasks[index]
    return await tasks[index]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected same immutable dynamic task-handle index reuse without reinit to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_immutable_projected_dynamic_task_handle_array_index_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker()
    return await tasks[slot.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected same immutable projected dynamic task-handle index reinitialization to restore that path, got {diagnostics:?}"
    );
}

#[test]
fn allows_conditionally_reinitializing_same_immutable_projected_dynamic_task_handle_array_index_before_branch_join()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool, index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    if flag {
        let first = await tasks[slot.value]
        tasks[slot.value] = worker()
    }
    return await tasks[slot.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional same immutable projected dynamic index reinitialization to clear maybe-moved facts, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_guard_refined_projected_dynamic_task_handle_array_index_before_literal_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(slot: Slot) -> Wrap {
    var tasks = [worker(), worker()]
    if slot.value == 0 {
        let first = await tasks[slot.value]
        tasks[0] = worker()
    }
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected guard-refined projected dynamic task-handle reinit through tasks[0] to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_immutable_projected_dynamic_task_handle_array_index_without_reinit()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let slot = Slot { value: index }
    let first = await tasks[slot.value]
    return await tasks[slot.value]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected same immutable projected dynamic task-handle index reuse without reinit to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_literal_backed_immutable_dynamic_task_handle_array_index_and_literal_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let tasks = [worker(), worker()]
    let index = 0
    let first = await tasks[index]
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected literal-backed immutable dynamic task-handle index reuse through tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_literal_backed_immutable_projected_dynamic_task_handle_array_index_and_literal_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let tasks = [worker(), worker()]
    let slot = Slot { value: 0 }
    let first = await tasks[slot.value]
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected literal-backed immutable projected dynamic task-handle index reuse through tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_literal_backed_immutable_projected_dynamic_task_handle_array_index_before_literal_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: 0 }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker()
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected literal-backed immutable projected dynamic task-handle index reinit to restore tasks[0], got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_immutable_alias_sourced_dynamic_task_handle_array_index_without_reinit()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let alias = index
    let first = await tasks[alias]
    return await tasks[index]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected same immutable alias-sourced dynamic task-handle index reuse to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_immutable_alias_sourced_projected_dynamic_task_handle_array_index_across_source_path()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    let first = await tasks[slot.value]
    tasks[index] = worker()
    return await tasks[slot.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected same immutable alias-sourced projected dynamic task-handle index reinit through source path to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_composed_stable_dynamic_task_handle_array_index_without_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(row: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let slots = [row, row]
    let first = await tasks[slots[row]]
    return await tasks[slots[row]]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected composed stable dynamic task-handle index reuse without reinit to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_composed_stable_dynamic_task_handle_array_index_after_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(row: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slots = [row, row]
    let first = await tasks[slots[row]]
    tasks[slots[row]] = worker()
    return await tasks[slots[row]]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected composed stable dynamic task-handle index reinit to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_alias_sourced_composed_dynamic_task_handle_array_index_without_reinit()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(row: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let slots = [row, row]
    let alias = slots
    let first = await tasks[alias[row]]
    return await tasks[slots[row]]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected alias-sourced composed dynamic task-handle index reuse to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_alias_sourced_composed_dynamic_task_handle_array_index_across_source_path()
{
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(row: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slots = [row, row]
    let alias = slots
    let first = await tasks[alias[row]]
    tasks[slots[row]] = worker()
    return await tasks[alias[row]]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected alias-sourced composed dynamic task-handle reinit through source path to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_const_backed_projected_root_dynamic_task_handle_array_index_and_literal_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pending = Pending {
        tasks: [worker(), worker()],
    }
    let first = await pending.tasks[INDEX]
    return await pending.tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `pending` was used after move".to_string()),
        "expected const-backed projected-root dynamic task-handle index reuse through pending.tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_const_backed_dynamic_task_handle_array_index_and_literal_reuse()
{
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let tasks = [worker(), worker()]
    let first = await tasks[INDEX]
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected const-backed dynamic task-handle index reuse through tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_static_backed_dynamic_task_handle_array_index_and_literal_reuse()
{
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

static INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let tasks = [worker(), worker()]
    let first = await tasks[INDEX]
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected static-backed dynamic task-handle index reuse through tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_same_branch_selected_const_backed_dynamic_task_handle_array_index_and_literal_reuse()
{
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

const NEXT: Int = 1
const INDEX: Int = if NEXT == 1 { 0 } else { 1 }

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let tasks = [worker(), worker()]
    let first = await tasks[INDEX]
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected branch-selected const-backed dynamic task-handle index reuse through tasks[0] to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_const_backed_projected_dynamic_task_handle_array_index_before_literal_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

const SLOT: Slot = Slot { value: 0 }

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[SLOT.value]
    tasks[0] = worker()
    return await tasks[SLOT.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected const-backed projected dynamic task-handle index reinit to restore tasks[0], got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_static_alias_backed_projected_dynamic_task_handle_array_index_before_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
use SLOT as INDEX_ALIAS

struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

static SLOT: Slot = Slot { value: 0 }

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[INDEX_ALIAS.value]
    tasks[0] = worker()
    return await tasks[INDEX_ALIAS.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected static alias-backed projected dynamic task-handle index reinit to restore tasks[0], got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_same_branch_selected_static_backed_projected_dynamic_task_handle_array_index_before_reuse()
 {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

static MATCH_KEY: Int = 1
static SLOT: Slot = match MATCH_KEY {
    1 => Slot { value: 0 },
    _ => Slot { value: 1 },
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[SLOT.value]
    tasks[0] = worker()
    return await tasks[SLOT.value]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected branch-selected static-backed projected dynamic task-handle index reinit to restore tasks[0], got {diagnostics:?}"
    );
}

#[test]
fn allows_dynamic_array_index_assignment_for_non_task_elements() {
    let diagnostics = diagnostic_messages(
        r#"
fn main(index: Int) -> Int {
    var values = [1, 2, 3]
    values[index] = 4
    return values[index]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected dynamic non-task array assignment to avoid borrowck regressions, got {diagnostics:?}"
    );
}

#[test]
fn allows_nested_dynamic_array_index_assignment_for_non_task_elements() {
    let diagnostics = diagnostic_messages(
        r#"
fn main(row: Int, col: Int) -> Int {
    var matrix = [[1, 2, 3], [4, 5, 6]]
    matrix[row][col] = 9
    return matrix[row][col]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected nested dynamic non-task array assignment to avoid borrowck regressions, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_aliased_direct_task_handle_without_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let task = worker()
    let alias = task
    let first = await alias
    return await task
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `task` was used after move".to_string()),
        "expected aliased direct task handle reuse through source local to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_aliased_direct_task_handle_through_source_local() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var task = worker()
    let alias = task
    let first = await alias
    task = worker()
    return await alias
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected aliased direct task handle reinit through source local to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_for_aliased_dynamic_task_handle_array_root_without_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let alias = tasks
    let first = await alias[index]
    return await tasks[index]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected aliased dynamic task-handle array root reuse through source local to be a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_reinitializing_aliased_projected_root_dynamic_task_handle_array_through_source_path() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let alias = pending.tasks
    let first = await alias[index]
    pending.tasks[index] = worker()
    return await alias[index]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected aliased projected-root dynamic task-handle reinit through source path to restore availability, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_when_aliased_direct_task_handle_is_repackaged_into_tuple() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let task = worker()
    let alias = task
    let first = await task
    let pair = (alias, worker())
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `task` was used after move".to_string()),
        "expected repackaging an aliased moved task handle into a tuple to report a definite move, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_move_when_aliased_dynamic_task_handle_root_is_repackaged_into_tuple() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let alias = tasks
    let first = await tasks[index]
    let pair = (alias[index], worker())
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected repackaging an aliased moved dynamic task-handle element into a tuple to report a definite move, got {diagnostics:?}"
    );
}

#[test]
fn allows_repackaging_aliased_projected_root_task_handle_after_source_path_reinit() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let alias = pending.tasks
    let first = await pending.tasks[index]
    pending.tasks[index] = worker()
    let pair = (alias[index], worker())
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected repackaging an aliased projected-root task handle after source-path reinit to succeed, got {diagnostics:?}"
    );
}

#[test]
fn renders_zero_sized_task_conditionally_spawned_async_call_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag)
}
"#,
    );

    assert!(
        rendered.contains("ownership choose"),
        "rendered output:\n{rendered}"
    );
    assert!(
        rendered.contains("consume(await task handle)"),
        "rendered output:\n{rendered}"
    );
}

#[test]
fn renders_zero_sized_task_reverse_branch_conditionally_spawned_async_call_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag)
}
"#,
    );

    assert!(
        rendered.contains("ownership choose"),
        "rendered output:\n{rendered}"
    );
    assert!(
        rendered.contains("consume(await task handle)"),
        "rendered output:\n{rendered}"
    );
}

#[test]
fn renders_zero_sized_task_reverse_branch_join_and_spawn_reinit_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        let running = spawn task;
        task = fresh_worker();
        await running
    }
    return await task
}
"#,
    );

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("consume(spawn task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_bound_helper_spawn_task_handle_consumes_for_debugging() {
    let rendered = render_output(
        r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(rendered.contains("consume(spawn task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_nested_task_handle_async_result_consumes_for_debugging() {
    let rendered = render_output(
        r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let pending = outer()
    let next = await pending
    return await next
}
"#,
    );

    assert_eq!(rendered.matches("consume(await task handle)").count(), 2);
}

#[test]
fn reports_use_after_awaiting_nested_task_handle_payload_twice() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let next = await outer()
    let first = await next
    return await next
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `next` was used after move".to_string()),
        "expected nested awaited task handle to remain move-tracked, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_sibling_task_handles_from_awaited_tuple_payload() {
    let diagnostics = diagnostic_messages(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn main() -> Int {
    let pending = outer()
    let pair = await pending
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected awaited tuple payload sibling task handles to stay independently usable, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_reawaiting_same_task_handle_from_awaited_tuple_payload() {
    let diagnostics = diagnostic_messages(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn main() -> Int {
    let pair = await outer()
    let first = await pair[0]
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `pair[0]` was used after move".to_string())
            || diagnostics.contains(&"local `pair` was used after move".to_string()),
        "expected awaited tuple payload projection to remain move-tracked, got {diagnostics:?}"
    );
}

#[test]
fn renders_awaited_struct_task_handle_payload_consumes_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

async fn main() -> Wrap {
    let pending = outer()
    let value = await pending
    await value.first
    return await value.second
}
"#,
    );

    assert_eq!(rendered.matches("consume(await task handle)").count(), 3);
}

#[test]
fn allows_awaiting_sibling_task_handles_from_awaited_array_payload() {
    let diagnostics = diagnostic_messages(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn main() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    let second = await tasks[1]
    return first + second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected awaited array payload sibling task handles to stay independently usable, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_reawaiting_same_task_handle_from_awaited_array_payload() {
    let diagnostics = diagnostic_messages(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn main() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `tasks[0]` was used after move".to_string())
            || diagnostics.contains(&"local `tasks` was used after move".to_string()),
        "expected awaited array payload projection to remain move-tracked, got {diagnostics:?}"
    );
}

#[test]
fn renders_awaited_array_task_handle_payload_consumes_for_debugging() {
    let rendered = render_output(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn main() -> Int {
    let tasks = outer()
    let pending = await tasks
    let first = await pending[0]
    return await pending[1]
}
"#,
    );

    assert_eq!(rendered.matches("consume(await task handle)").count(), 3);
}

#[test]
fn allows_awaiting_nested_aggregate_task_handles_from_awaited_payload() {
    let diagnostics = diagnostic_messages(
        r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected nested awaited aggregate payload task handles to stay independently usable, got {diagnostics:?}"
    );
}

#[test]
fn reports_use_after_reawaiting_same_nested_aggregate_task_handle_payload() {
    let diagnostics = diagnostic_messages(
        r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    return await pending[0].task
}
"#,
    );

    assert!(
        diagnostics.contains(&"local `pending[0].task` was used after move".to_string())
            || diagnostics.contains(&"local `pending` was used after move".to_string()),
        "expected nested aggregate payload projection to remain move-tracked, got {diagnostics:?}"
    );
}

#[test]
fn renders_nested_aggregate_task_handle_payload_consumes_for_debugging() {
    let rendered = render_output(
        r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let tasks = outer()
    let pending = await tasks
    let first = await pending[0].task
    return await pending[1].task
}
"#,
    );

    assert_eq!(rendered.matches("consume(await task handle)").count(), 3);
}

#[test]
fn renders_bound_zero_sized_helper_spawn_task_handle_consumes_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(rendered.contains("consume(spawn task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_helper_task_handle_argument_consumes_for_debugging() {
    let rendered = render_output(
        r#"
fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let forwarded = forward(task)
    return await forwarded
}
"#,
    );

    assert!(rendered.contains("consume(call task handle argument)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_zero_sized_helper_task_handle_argument_consumes_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let task = worker()
    let forwarded = forward(task)
    return await forwarded
}
"#,
    );

    assert!(rendered.contains("consume(call task handle argument)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_return_task_handle_consumes_for_debugging() {
    let rendered = render_output(
        r#"
fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let forwarded = forward(worker())
    return await forwarded
}
"#,
    );

    assert!(rendered.contains("consume(return task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_zero_sized_return_task_handle_consumes_for_debugging() {
    let rendered = render_output(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let forwarded = forward(worker())
    return await forwarded
}
"#,
    );

    assert!(rendered.contains("consume(return task handle)"));
    assert!(rendered.contains("consume(await task handle)"));
}

#[test]
fn renders_closure_escape_facts_for_debugging() {
    let rendered = render_output(
        r#"
fn apply(f: () -> Int) -> Int {
    return f()
}

fn make_adder(delta: Int) -> (Int) -> Int {
    let add = move (x) => x + delta
    return add
}

fn main() -> Int {
    let base = 1
    let first = move () => base
    let wrapped = () => first
    return apply(first)
}
"#,
    );

    assert!(rendered.contains("closures:"));
    assert!(rendered.contains("escapes=[return@"));
    assert!(rendered.contains("call-arg@"));
    assert!(rendered.contains("captured-by-cl"));
    assert!(rendered.contains("escapes=[local-only]"));
}
