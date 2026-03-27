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
    AsyncFrameAlloc,
    AsyncTaskCreate,
    ExecutorSpawn,
    TaskAwait,
    AsyncIterNext,
}

impl RuntimeHook {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::AsyncFrameAlloc => "async-frame-alloc",
            Self::AsyncTaskCreate => "async-task-create",
            Self::ExecutorSpawn => "executor-spawn",
            Self::TaskAwait => "task-await",
            Self::AsyncIterNext => "async-iter-next",
        }
    }

    pub const fn symbol_name(self) -> &'static str {
        match self {
            Self::AsyncFrameAlloc => "qlrt_async_frame_alloc",
            Self::AsyncTaskCreate => "qlrt_async_task_create",
            Self::ExecutorSpawn => "qlrt_executor_spawn",
            Self::TaskAwait => "qlrt_task_await",
            Self::AsyncIterNext => "qlrt_async_iter_next",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::AsyncFrameAlloc => {
                "allocating heap-backed async frames for `async fn` entry wrappers"
            }
            Self::AsyncTaskCreate => "materializing an executable task from an `async fn` body",
            Self::ExecutorSpawn => "submitting a task to the configured runtime executor",
            Self::TaskAwait => "waiting for an asynchronous task result",
            Self::AsyncIterNext => "advancing the next item in a `for await` iteration",
        }
    }
}

/// Minimal ABI-level type vocabulary for future runtime hook lowering.
///
/// This intentionally stays tiny and opaque at first so the compiler can
/// freeze symbol names plus a first LLVM-facing signature shape without
/// committing to concrete task/join/result/frame layouts yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RuntimeAbiType {
    Void,
    Bool,
    I64,
    Ptr,
}

impl RuntimeAbiType {
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "bool",
            Self::I64 => "i64",
            Self::Ptr => "ptr",
        }
    }

    pub const fn llvm_ir(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "i1",
            Self::I64 => "i64",
            Self::Ptr => "ptr",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct RuntimeHookParam {
    pub name: &'static str,
    pub ty: RuntimeAbiType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeHookSignature {
    pub hook: RuntimeHook,
    pub return_type: RuntimeAbiType,
    pub params: &'static [RuntimeHookParam],
}

impl RuntimeHookSignature {
    pub const fn calling_convention(self) -> &'static str {
        "ccc"
    }

    pub fn render_contract(self) -> String {
        let params = self
            .params
            .iter()
            .map(|param| format!("{}: {}", param.name, param.ty.stable_name()))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} {}({params}) -> {}",
            self.calling_convention(),
            self.hook.symbol_name(),
            self.return_type.stable_name(),
        )
    }

    pub fn render_llvm_declaration(self) -> String {
        let params = self
            .params
            .iter()
            .map(|param| param.ty.llvm_ir())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "declare {} @{}({params})",
            self.return_type.llvm_ir(),
            self.hook.symbol_name(),
        )
    }
}

const ASYNC_TASK_CREATE_PARAMS: &[RuntimeHookParam] = &[
    RuntimeHookParam {
        name: "entry",
        ty: RuntimeAbiType::Ptr,
    },
    RuntimeHookParam {
        name: "frame",
        ty: RuntimeAbiType::Ptr,
    },
];
const ASYNC_FRAME_ALLOC_PARAMS: &[RuntimeHookParam] = &[
    RuntimeHookParam {
        name: "size",
        ty: RuntimeAbiType::I64,
    },
    RuntimeHookParam {
        name: "align",
        ty: RuntimeAbiType::I64,
    },
];
const EXECUTOR_SPAWN_PARAMS: &[RuntimeHookParam] = &[
    RuntimeHookParam {
        name: "executor",
        ty: RuntimeAbiType::Ptr,
    },
    RuntimeHookParam {
        name: "task",
        ty: RuntimeAbiType::Ptr,
    },
];
const TASK_AWAIT_PARAMS: &[RuntimeHookParam] = &[RuntimeHookParam {
    name: "join_handle",
    ty: RuntimeAbiType::Ptr,
}];
const ASYNC_ITER_NEXT_PARAMS: &[RuntimeHookParam] = &[RuntimeHookParam {
    name: "iterator",
    ty: RuntimeAbiType::Ptr,
}];

const ASYNC_FUNCTION_BODY_HOOKS: &[RuntimeHook] =
    &[RuntimeHook::AsyncFrameAlloc, RuntimeHook::AsyncTaskCreate];
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

pub const fn runtime_hook_signature(hook: RuntimeHook) -> RuntimeHookSignature {
    match hook {
        RuntimeHook::AsyncFrameAlloc => RuntimeHookSignature {
            hook,
            return_type: RuntimeAbiType::Ptr,
            params: ASYNC_FRAME_ALLOC_PARAMS,
        },
        RuntimeHook::AsyncTaskCreate => RuntimeHookSignature {
            hook,
            return_type: RuntimeAbiType::Ptr,
            params: ASYNC_TASK_CREATE_PARAMS,
        },
        RuntimeHook::ExecutorSpawn => RuntimeHookSignature {
            hook,
            return_type: RuntimeAbiType::Ptr,
            params: EXECUTOR_SPAWN_PARAMS,
        },
        RuntimeHook::TaskAwait => RuntimeHookSignature {
            hook,
            return_type: RuntimeAbiType::Ptr,
            params: TASK_AWAIT_PARAMS,
        },
        RuntimeHook::AsyncIterNext => RuntimeHookSignature {
            hook,
            return_type: RuntimeAbiType::Ptr,
            params: ASYNC_ITER_NEXT_PARAMS,
        },
    }
}

pub fn collect_runtime_hook_signatures<I>(capabilities: I) -> Vec<RuntimeHookSignature>
where
    I: IntoIterator<Item = RuntimeCapability>,
{
    collect_runtime_hooks(capabilities)
        .into_iter()
        .map(runtime_hook_signature)
        .collect()
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
