use std::cell::RefCell;
use std::rc::Rc;

use ql_runtime::{
    Executor, InlineExecutor, RuntimeAbiType, RuntimeCapability, RuntimeHook,
    collect_runtime_hook_signatures, collect_runtime_hooks, runtime_hook_signature,
    runtime_hooks_for_capability,
};

// ---------------------------------------------------------------------------
// Hook lifecycle contract tests (P7.2 Task 1)
//
// These tests assert the exact ABI signatures that uphold the backend's
// "opaque ptr → load → release" assumption for async task results.
// ---------------------------------------------------------------------------

/// The full async task creation lifecycle uses hooks in this order:
///   AsyncFrameAlloc → AsyncTaskCreate → ExecutorSpawn → TaskAwait → TaskResultRelease
///
/// This test locks the return types that the backend depends on at each step.
#[test]
fn hook_lifecycle_create_await_result_load_release_abi_contract() {
    // Frame alloc: allocates the parameter frame, returns ptr to frame.
    let frame_alloc = runtime_hook_signature(RuntimeHook::AsyncFrameAlloc);
    assert_eq!(frame_alloc.return_type, RuntimeAbiType::Ptr,
        "AsyncFrameAlloc must return ptr (caller writes params into it)");

    // Task create: consumes entry fn ptr + frame ptr, returns opaque task ptr.
    let task_create = runtime_hook_signature(RuntimeHook::AsyncTaskCreate);
    assert_eq!(task_create.return_type, RuntimeAbiType::Ptr,
        "AsyncTaskCreate must return ptr (opaque task handle, caller owns until spawn)");

    // Executor spawn: takes executor ptr + task ptr, returns join handle ptr.
    let exec_spawn = runtime_hook_signature(RuntimeHook::ExecutorSpawn);
    assert_eq!(exec_spawn.return_type, RuntimeAbiType::Ptr,
        "ExecutorSpawn must return ptr (join handle, caller owns until await)");

    // Task await: consumes join handle, returns result_ptr.
    // CRITICAL: the backend immediately does `load <RetTy>, ptr result_ptr` after
    // this call.  The return type MUST be Ptr and the pointed-to region must be
    // a contiguous, naturally aligned payload of the async return type.
    let task_await = runtime_hook_signature(RuntimeHook::TaskAwait);
    assert_eq!(task_await.return_type, RuntimeAbiType::Ptr,
        "TaskAwait must return ptr — backend loads result payload directly from it");
    assert_eq!(task_await.params.len(), 1,
        "TaskAwait takes exactly one argument (join_handle: ptr)");
    assert_eq!(task_await.params[0].name, "join_handle");
    assert_eq!(task_await.params[0].ty, RuntimeAbiType::Ptr);

    // Result release: frees the result_ptr after the value has been extracted.
    // Must return void — no observable output.
    let task_result_release = runtime_hook_signature(RuntimeHook::TaskResultRelease);
    assert_eq!(task_result_release.return_type, RuntimeAbiType::Void,
        "TaskResultRelease must return void — callee frees backing memory");
    assert_eq!(task_result_release.params.len(), 1,
        "TaskResultRelease takes exactly one argument (result: ptr)");
    assert_eq!(task_result_release.params[0].name, "result");
    assert_eq!(task_result_release.params[0].ty, RuntimeAbiType::Ptr);
}

/// The backend emits the full lifecycle as a sequence of LLVM declarations.
/// This test locks the exact LLVM IR declaration strings that must appear in
/// any compiled module that uses the async task lifecycle.
#[test]
fn hook_lifecycle_full_llvm_declaration_sequence_is_stable() {
    let lifecycle_hooks = [
        RuntimeHook::AsyncFrameAlloc,
        RuntimeHook::AsyncTaskCreate,
        RuntimeHook::ExecutorSpawn,
        RuntimeHook::TaskAwait,
        RuntimeHook::TaskResultRelease,
    ];

    let declarations: Vec<String> = lifecycle_hooks
        .iter()
        .map(|&hook| runtime_hook_signature(hook).render_llvm_declaration())
        .collect();

    assert_eq!(
        declarations,
        vec![
            "declare ptr @qlrt_async_frame_alloc(i64, i64)",
            "declare ptr @qlrt_async_task_create(ptr, ptr)",
            "declare ptr @qlrt_executor_spawn(ptr, ptr)",
            "declare ptr @qlrt_task_await(ptr)",
            "declare void @qlrt_task_result_release(ptr)",
        ],
        "LLVM declarations for the async task lifecycle must remain stable"
    );
}

/// The async iteration hook is a separate lifecycle: iterator_ptr (borrowed) → next item ptr.
/// Null return signals end-of-iteration; non-null is caller-owned.
#[test]
fn async_iter_next_abi_contract_is_stable() {
    let iter_next = runtime_hook_signature(RuntimeHook::AsyncIterNext);

    assert_eq!(iter_next.return_type, RuntimeAbiType::Ptr,
        "AsyncIterNext returns ptr (null = end-of-iteration, non-null = caller-owned item)");
    assert_eq!(iter_next.params.len(), 1,
        "AsyncIterNext takes exactly one argument (iterator: ptr)");
    assert_eq!(iter_next.params[0].name, "iterator");
    assert_eq!(iter_next.params[0].ty, RuntimeAbiType::Ptr,
        "AsyncIterNext borrows the iterator ptr for the duration of the call");
    assert_eq!(
        iter_next.render_llvm_declaration(),
        "declare ptr @qlrt_async_iter_next(ptr)"
    );
}

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
