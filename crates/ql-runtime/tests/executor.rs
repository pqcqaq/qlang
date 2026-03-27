use std::cell::RefCell;
use std::rc::Rc;

use ql_runtime::{Executor, InlineExecutor, RuntimeCapability};

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
