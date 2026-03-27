/// Minimal task contract for the Phase 7 runtime foundation.
///
/// This intentionally models run-to-completion work instead of committing the
/// compiler to a concrete Rust `Future` representation before the Qlang async
/// runtime/effect surface is frozen.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RuntimeCapability {
    AsyncFunctionBodies,
    TaskSpawn,
    TaskAwait,
    AsyncIteration,
}

impl RuntimeCapability {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::AsyncFunctionBodies => "async-function-bodies",
            Self::TaskSpawn => "task-spawn",
            Self::TaskAwait => "task-await",
            Self::AsyncIteration => "async-iteration",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::AsyncFunctionBodies => {
                "lowering and executing body-bearing `async fn` definitions"
            }
            Self::TaskSpawn => "spawning asynchronous tasks",
            Self::TaskAwait => "awaiting asynchronous task results",
            Self::AsyncIteration => "driving `for await` iteration",
        }
    }
}

/// Shared runtime hook surface reserved for future backend/runtime ABI work.
///
/// These names intentionally stay at the contract level instead of committing
/// the compiler to concrete Rust `Future` shapes, scheduler APIs, or stream
/// protocols before Phase 7 lowering is designed end to end.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RuntimeHook {
    AsyncTaskCreate,
    ExecutorSpawn,
    TaskAwait,
    AsyncIterNext,
}

impl RuntimeHook {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::AsyncTaskCreate => "async-task-create",
            Self::ExecutorSpawn => "executor-spawn",
            Self::TaskAwait => "task-await",
            Self::AsyncIterNext => "async-iter-next",
        }
    }

    pub const fn symbol_name(self) -> &'static str {
        match self {
            Self::AsyncTaskCreate => "qlrt_async_task_create",
            Self::ExecutorSpawn => "qlrt_executor_spawn",
            Self::TaskAwait => "qlrt_task_await",
            Self::AsyncIterNext => "qlrt_async_iter_next",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::AsyncTaskCreate => "materializing an executable task from an `async fn` body",
            Self::ExecutorSpawn => "submitting a task to the configured runtime executor",
            Self::TaskAwait => "waiting for an asynchronous task result",
            Self::AsyncIterNext => "advancing the next item in a `for await` iteration",
        }
    }
}

const ASYNC_FUNCTION_BODY_HOOKS: &[RuntimeHook] = &[RuntimeHook::AsyncTaskCreate];
const TASK_SPAWN_HOOKS: &[RuntimeHook] = &[RuntimeHook::ExecutorSpawn];
const TASK_AWAIT_HOOKS: &[RuntimeHook] = &[RuntimeHook::TaskAwait];
const ASYNC_ITERATION_HOOKS: &[RuntimeHook] = &[RuntimeHook::AsyncIterNext];

pub const fn runtime_hooks_for_capability(capability: RuntimeCapability) -> &'static [RuntimeHook] {
    match capability {
        RuntimeCapability::AsyncFunctionBodies => ASYNC_FUNCTION_BODY_HOOKS,
        RuntimeCapability::TaskSpawn => TASK_SPAWN_HOOKS,
        RuntimeCapability::TaskAwait => TASK_AWAIT_HOOKS,
        RuntimeCapability::AsyncIteration => ASYNC_ITERATION_HOOKS,
    }
}

pub fn collect_runtime_hooks<I>(capabilities: I) -> Vec<RuntimeHook>
where
    I: IntoIterator<Item = RuntimeCapability>,
{
    let mut hooks = Vec::new();
    for capability in capabilities {
        for &hook in runtime_hooks_for_capability(capability) {
            if !hooks.contains(&hook) {
                hooks.push(hook);
            }
        }
    }
    hooks.sort();
    hooks
}

pub trait Task {
    type Output;

    fn run(self) -> Self::Output;
}

impl<F, T> Task for F
where
    F: FnOnce() -> T,
{
    type Output = T;

    fn run(self) -> Self::Output {
        self()
    }
}

/// Handle returned by an executor after spawning a task.
pub trait JoinHandle {
    type Output;

    fn join(self) -> Self::Output;
}

/// Minimal executor abstraction for Phase 7.
///
/// The contract stays deliberately small:
/// - executors accept a run-to-completion task
/// - spawning returns a join handle
/// - no polling, cancellation, or scheduler hints are promised yet
pub trait Executor {
    type Handle<T>: JoinHandle<Output = T>;

    fn spawn<T, Work>(&self, task: Work) -> Self::Handle<T>
    where
        T: 'static,
        Work: Task<Output = T> + 'static;
}

/// Join handle produced by [`InlineExecutor`].
#[derive(Debug)]
pub struct ReadyJoinHandle<T> {
    output: T,
}

impl<T> ReadyJoinHandle<T> {
    pub fn new(output: T) -> Self {
        Self { output }
    }

    pub fn join(self) -> T {
        self.output
    }
}

impl<T> JoinHandle for ReadyJoinHandle<T> {
    type Output = T;

    fn join(self) -> Self::Output {
        ReadyJoinHandle::join(self)
    }
}

/// Deterministic single-thread executor used as the first runtime baseline.
///
/// It executes tasks immediately on the caller thread and returns a ready join
/// handle. This gives later lowering/codegen work a stable abstraction point
/// without forcing the repository to pick a full async scheduler yet.
#[derive(Clone, Copy, Debug, Default)]
pub struct InlineExecutor;

impl InlineExecutor {
    pub fn block_on<T, Work>(&self, task: Work) -> T
    where
        T: 'static,
        Work: Task<Output = T> + 'static,
    {
        self.spawn(task).join()
    }
}

impl Executor for InlineExecutor {
    type Handle<T> = ReadyJoinHandle<T>;

    fn spawn<T, Work>(&self, task: Work) -> Self::Handle<T>
    where
        T: 'static,
        Work: Task<Output = T> + 'static,
    {
        ReadyJoinHandle::new(task.run())
    }
}
