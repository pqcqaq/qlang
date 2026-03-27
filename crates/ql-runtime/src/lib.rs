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
