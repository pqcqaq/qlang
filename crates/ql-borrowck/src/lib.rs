mod analyze;
mod render;

use ql_diagnostics::Diagnostic;
use ql_mir::{BodyOwner, ClosureId, LocalId};
use ql_span::Span;

pub use analyze::analyze_module;
pub use render::render_result;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BorrowckResult {
    diagnostics: Vec<Diagnostic>,
    bodies: Vec<BodyFacts>,
}

impl BorrowckResult {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn bodies(&self) -> &[BodyFacts] {
        &self.bodies
    }

    pub(crate) fn push_body(&mut self, body: BodyFacts) {
        self.bodies.push(body);
    }

    pub(crate) fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyFacts {
    pub owner: BodyOwner,
    pub name: String,
    pub blocks: Vec<BlockFacts>,
    pub events: Vec<LocalEvent>,
    pub closures: Vec<ClosureFacts>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockFacts {
    pub block_index: usize,
    pub entry_states: Vec<LocalState>,
    pub exit_states: Vec<LocalState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalEvent {
    pub span: Span,
    pub local: LocalId,
    pub kind: LocalEventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureFacts {
    pub closure: ClosureId,
    pub escapes: Vec<ClosureEscape>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClosureEscape {
    pub span: Span,
    pub kind: ClosureEscapeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ClosureEscapeKind {
    Return,
    CallArgument,
    CallCallee,
    CapturedByClosure { outer: ClosureId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalEventKind {
    Read,
    Write,
    Consume(MoveReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalState {
    Unavailable,
    Available,
    Moved(MoveInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveInfo {
    pub certainty: MoveCertainty,
    pub origins: Vec<MoveOrigin>,
    pub path_moves: Vec<PathMoveInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoveCertainty {
    Definite,
    Maybe,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MovePathSegment {
    Field(String),
    TupleIndex(usize),
    ArrayIndex(usize),
    DynamicArrayIndexLocal(LocalId),
    DynamicArrayIndex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathMoveInfo {
    pub path: Vec<MovePathSegment>,
    pub certainty: MoveCertainty,
    pub origins: Vec<MoveOrigin>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MoveOrigin {
    pub span: Span,
    pub reason: MoveReason,
    pub path: Vec<MovePathSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MoveReason {
    MoveSelfMethod { method_name: String },
    MoveClosureCapture,
    AwaitTaskHandle,
    SpawnTaskHandle,
    CallTaskHandleArgument,
    ReturnTaskHandle,
}
