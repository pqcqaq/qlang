use std::cell::RefCell;
use std::rc::Rc;

use ql_runtime::{
    Executor, InlineExecutor, RuntimeAbiType, RuntimeCapability, RuntimeHook,
    collect_runtime_hook_signatures, collect_runtime_hooks, runtime_hook_signature,
    runtime_hooks_for_capability,
};

#[test]
fn runtime_capabilities_expose_stable_names() {
    let capabilities = [
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ];

    assert_eq!(
        capabilities
            .into_iter()
            .map(RuntimeCapability::stable_name)
            .collect::<Vec<_>>(),
        vec![
            "async-function-bodies",
            "task-spawn",
            "task-await",
            "async-iteration",
        ]
    );
}

#[test]
fn runtime_hooks_expose_stable_names_and_symbols() {
    let hooks = [
        RuntimeHook::AsyncFrameAlloc,
        RuntimeHook::AsyncTaskCreate,
        RuntimeHook::ExecutorSpawn,
        RuntimeHook::TaskAwait,
        RuntimeHook::TaskResultRelease,
        RuntimeHook::AsyncIterNext,
    ];

    assert_eq!(
        hooks
            .into_iter()
            .map(|hook| (hook.stable_name(), hook.symbol_name()))
            .collect::<Vec<_>>(),
        vec![
            ("async-frame-alloc", "qlrt_async_frame_alloc"),
            ("async-task-create", "qlrt_async_task_create"),
            ("executor-spawn", "qlrt_executor_spawn"),
            ("task-await", "qlrt_task_await"),
            ("task-result-release", "qlrt_task_result_release"),
            ("async-iter-next", "qlrt_async_iter_next"),
        ]
    );
}

#[test]
fn runtime_hook_signatures_expose_stable_contract_strings() {
    let signature = runtime_hook_signature(RuntimeHook::AsyncFrameAlloc);

    assert_eq!(signature.calling_convention(), "ccc");
    assert_eq!(signature.return_type, RuntimeAbiType::Ptr);
    assert_eq!(
        signature.render_contract(),
        "ccc qlrt_async_frame_alloc(size: i64, align: i64) -> ptr"
    );
    assert_eq!(
        signature.render_llvm_declaration(),
        "declare ptr @qlrt_async_frame_alloc(i64, i64)"
    );
}

#[test]
fn async_task_create_signature_keeps_entry_and_frame_contract() {
    let signature = runtime_hook_signature(RuntimeHook::AsyncTaskCreate);

    assert_eq!(signature.calling_convention(), "ccc");
    assert_eq!(signature.return_type, RuntimeAbiType::Ptr);
    assert_eq!(
        signature.render_contract(),
        "ccc qlrt_async_task_create(entry: ptr, frame: ptr) -> ptr"
    );
    assert_eq!(
        signature.render_llvm_declaration(),
        "declare ptr @qlrt_async_task_create(ptr, ptr)"
    );
}

#[test]
fn task_result_release_signature_keeps_payload_release_contract() {
    let signature = runtime_hook_signature(RuntimeHook::TaskResultRelease);

    assert_eq!(signature.calling_convention(), "ccc");
    assert_eq!(signature.return_type, RuntimeAbiType::Void);
    assert_eq!(
        signature.render_contract(),
        "ccc qlrt_task_result_release(result: ptr) -> void"
    );
    assert_eq!(
        signature.render_llvm_declaration(),
        "declare void @qlrt_task_result_release(ptr)"
    );
}

#[test]
fn collect_runtime_hook_signatures_preserves_sorted_hook_plan() {
    let signatures = collect_runtime_hook_signatures([
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::AsyncIteration,
    ]);

    assert_eq!(
        signatures
            .into_iter()
            .map(|signature| signature.render_contract())
            .collect::<Vec<_>>(),
        vec![
            "ccc qlrt_async_frame_alloc(size: i64, align: i64) -> ptr",
            "ccc qlrt_async_task_create(entry: ptr, frame: ptr) -> ptr",
            "ccc qlrt_executor_spawn(executor: ptr, task: ptr) -> ptr",
            "ccc qlrt_task_await(join_handle: ptr) -> ptr",
            "ccc qlrt_task_result_release(result: ptr) -> void",
            "ccc qlrt_async_iter_next(iterator: ptr) -> ptr",
        ]
    );
}

#[test]
fn runtime_capabilities_map_to_shared_hook_contracts() {
    assert_eq!(
        runtime_hooks_for_capability(RuntimeCapability::AsyncFunctionBodies),
        &[RuntimeHook::AsyncFrameAlloc, RuntimeHook::AsyncTaskCreate]
    );
    assert_eq!(
        runtime_hooks_for_capability(RuntimeCapability::TaskSpawn),
        &[RuntimeHook::ExecutorSpawn]
    );
    assert_eq!(
        runtime_hooks_for_capability(RuntimeCapability::TaskAwait),
        &[RuntimeHook::TaskAwait, RuntimeHook::TaskResultRelease]
    );
    assert_eq!(
        runtime_hooks_for_capability(RuntimeCapability::AsyncIteration),
        &[RuntimeHook::AsyncIterNext]
    );
}

#[test]
fn collect_runtime_hooks_dedupes_and_orders_the_contract_surface() {
    let hooks = collect_runtime_hooks([
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::AsyncIteration,
    ]);

    assert_eq!(
        hooks,
        vec![
            RuntimeHook::AsyncFrameAlloc,
            RuntimeHook::AsyncTaskCreate,
            RuntimeHook::ExecutorSpawn,
            RuntimeHook::TaskAwait,
            RuntimeHook::TaskResultRelease,
            RuntimeHook::AsyncIterNext,
        ]
    );
}

#[test]
fn inline_executor_runs_spawned_tasks_to_completion() {
    let executor = InlineExecutor;

    let handle = executor.spawn(|| 40 + 2);

    assert_eq!(handle.join(), 42);
}

#[test]
fn inline_executor_block_on_reuses_the_same_run_to_completion_contract() {
    let executor = InlineExecutor;

    let result = executor.block_on(|| String::from("qlang"));

    assert_eq!(result, "qlang");
}

#[test]
fn inline_executor_executes_tasks_in_spawn_order() {
    let executor = InlineExecutor;
    let events = Rc::new(RefCell::new(Vec::new()));

    let first_events = Rc::clone(&events);
    let first = executor.spawn(move || {
        first_events.borrow_mut().push("first");
        1
    });

    let second_events = Rc::clone(&events);
    let second = executor.spawn(move || {
        second_events.borrow_mut().push("second");
        2
    });

    assert_eq!(first.join(), 1);
    assert_eq!(second.join(), 2);
    assert_eq!(*events.borrow(), vec!["first", "second"]);
}
