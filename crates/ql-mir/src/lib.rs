mod ids;
mod lower;
mod render;

use std::collections::HashMap;

use ql_ast::{BinaryOp, Path, UnaryOp};
use ql_hir::{ExprId, FunctionRef, ItemId, PatternId};
use ql_span::Span;

pub use ids::{BasicBlockId, BodyId, CleanupId, ClosureId, LocalId, ScopeId, StatementId};
pub use lower::lower_module;
pub use render::render_module;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MirModule {
    body_order: Vec<BodyId>,
    body_data: Vec<MirBody>,
    body_lookup: HashMap<BodyOwner, BodyId>,
}

impl MirModule {
    pub fn bodies(&self) -> &[BodyId] {
        &self.body_order
    }

    pub fn body(&self, id: BodyId) -> &MirBody {
        &self.body_data[id.index()]
    }

    pub fn body_for_owner(&self, owner: BodyOwner) -> Option<&MirBody> {
        self.body_lookup
            .get(&owner)
            .copied()
            .map(|id| self.body(id))
    }

    pub(crate) fn alloc_body(&mut self, owner: BodyOwner, body: MirBody) -> BodyId {
        let id = BodyId::from_index(self.body_data.len());
        self.body_order.push(id);
        self.body_data.push(body);
        self.body_lookup.insert(owner, id);
        id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyOwner {
    Item(ItemId),
    TraitMethod { item: ItemId, index: usize },
    ImplMethod { item: ItemId, index: usize },
    ExtendMethod { item: ItemId, index: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirBody {
    pub owner: BodyOwner,
    pub name: String,
    pub span: Span,
    pub entry: BasicBlockId,
    pub return_block: BasicBlockId,
    pub return_local: LocalId,
    pub root_scope: ScopeId,
    local_data: Vec<LocalDecl>,
    block_data: Vec<BasicBlock>,
    statement_data: Vec<Statement>,
    scope_data: Vec<MirScope>,
    cleanup_data: Vec<CleanupAction>,
    closure_data: Vec<ClosureDecl>,
}

impl MirBody {
    pub fn local(&self, id: LocalId) -> &LocalDecl {
        &self.local_data[id.index()]
    }

    pub fn locals(&self) -> &[LocalDecl] {
        &self.local_data
    }

    pub fn local_ids(&self) -> impl Iterator<Item = LocalId> + '_ {
        (0..self.local_data.len()).map(LocalId::from_index)
    }

    pub fn block(&self, id: BasicBlockId) -> &BasicBlock {
        &self.block_data[id.index()]
    }

    pub fn blocks(&self) -> &[BasicBlock] {
        &self.block_data
    }

    pub fn block_ids(&self) -> impl Iterator<Item = BasicBlockId> + '_ {
        (0..self.block_data.len()).map(BasicBlockId::from_index)
    }

    pub fn statement(&self, id: StatementId) -> &Statement {
        &self.statement_data[id.index()]
    }

    pub fn statements(&self) -> &[Statement] {
        &self.statement_data
    }

    pub fn scope(&self, id: ScopeId) -> &MirScope {
        &self.scope_data[id.index()]
    }

    pub fn scopes(&self) -> &[MirScope] {
        &self.scope_data
    }

    pub fn cleanup(&self, id: CleanupId) -> &CleanupAction {
        &self.cleanup_data[id.index()]
    }

    pub fn cleanups(&self) -> &[CleanupAction] {
        &self.cleanup_data
    }

    pub fn closure(&self, id: ClosureId) -> &ClosureDecl {
        &self.closure_data[id.index()]
    }

    pub fn closures(&self) -> &[ClosureDecl] {
        &self.closure_data
    }

    pub fn closure_ids(&self) -> impl Iterator<Item = ClosureId> + '_ {
        (0..self.closure_data.len()).map(ClosureId::from_index)
    }

    pub(crate) fn alloc_local(&mut self, local: LocalDecl) -> LocalId {
        let id = LocalId::from_index(self.local_data.len());
        self.local_data.push(local);
        id
    }

    pub(crate) fn alloc_block(&mut self, block: BasicBlock) -> BasicBlockId {
        let id = BasicBlockId::from_index(self.block_data.len());
        self.block_data.push(block);
        id
    }

    pub(crate) fn block_mut(&mut self, id: BasicBlockId) -> &mut BasicBlock {
        &mut self.block_data[id.index()]
    }

    pub(crate) fn alloc_statement(&mut self, statement: Statement) -> StatementId {
        let id = StatementId::from_index(self.statement_data.len());
        self.statement_data.push(statement);
        id
    }

    pub(crate) fn alloc_scope(&mut self, scope: MirScope) -> ScopeId {
        let id = ScopeId::from_index(self.scope_data.len());
        self.scope_data.push(scope);
        id
    }

    pub(crate) fn scope_mut(&mut self, id: ScopeId) -> &mut MirScope {
        &mut self.scope_data[id.index()]
    }

    pub(crate) fn alloc_cleanup(&mut self, cleanup: CleanupAction) -> CleanupId {
        let id = CleanupId::from_index(self.cleanup_data.len());
        self.cleanup_data.push(cleanup);
        id
    }

    pub(crate) fn alloc_closure(&mut self, closure: ClosureDecl) -> ClosureId {
        let id = ClosureId::from_index(self.closure_data.len());
        self.closure_data.push(closure);
        id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalDecl {
    pub name: String,
    pub span: Span,
    pub mutable: bool,
    pub kind: LocalKind,
    pub origin: LocalOrigin,
    pub scope: ScopeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Return,
    Param,
    Binding,
    Temp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOrigin {
    ReturnSlot,
    Receiver,
    Param { index: usize },
    Binding(ql_hir::LocalId),
    Temp { ordinal: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasicBlock {
    pub span: Span,
    pub statements: Vec<StatementId>,
    pub terminator: Terminator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Assign {
        place: Place,
        value: Rvalue,
    },
    BindPattern {
        pattern: PatternId,
        source: Operand,
        mutable: bool,
    },
    Eval {
        value: Rvalue,
    },
    StorageLive {
        local: LocalId,
    },
    StorageDead {
        local: LocalId,
    },
    RegisterCleanup {
        cleanup: CleanupId,
    },
    RunCleanup {
        cleanup: CleanupId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TerminatorKind {
    Goto {
        target: BasicBlockId,
    },
    Branch {
        condition: Operand,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    Match {
        scrutinee: Operand,
        arms: Vec<MatchArmTarget>,
        else_target: BasicBlockId,
    },
    ForLoop {
        iterable: Operand,
        item_local: LocalId,
        is_await: bool,
        body_target: BasicBlockId,
        exit_target: BasicBlockId,
    },
    Return,
    Terminate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArmTarget {
    pub pattern: PatternId,
    pub guard: Option<ExprId>,
    pub target: BasicBlockId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub base: LocalId,
    pub projections: Vec<ProjectionElem>,
}

impl Place {
    pub fn local(local: LocalId) -> Self {
        Self {
            base: local,
            projections: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionElem {
    Field(String),
    TupleIndex(usize),
    Index(Box<Operand>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    Place(Place),
    Constant(Constant),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constant {
    Integer(String),
    String { value: String, is_format: bool },
    Bool(bool),
    None,
    Void,
    Function { function: FunctionRef, name: String },
    Item { item: ItemId, name: String },
    Import(Path),
    UnresolvedName(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rvalue {
    Use(Operand),
    Tuple(Vec<Operand>),
    Array(Vec<Operand>),
    Call {
        callee: Operand,
        args: Vec<CallArgument>,
    },
    Binary {
        left: Operand,
        op: BinaryOp,
        right: Operand,
    },
    Unary {
        op: UnaryOp,
        operand: Operand,
    },
    AggregateStruct {
        path: Path,
        fields: Vec<AggregateField>,
    },
    Closure {
        closure: ClosureId,
    },
    Question(Operand),
    OpaqueExpr(ExprId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallArgument {
    pub name: Option<String>,
    pub value: Operand,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AggregateField {
    pub name: String,
    pub value: Operand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosureCapture {
    pub local: LocalId,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureDecl {
    pub span: Span,
    pub is_move: bool,
    pub params: Vec<String>,
    pub captures: Vec<ClosureCapture>,
    pub body: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirScope {
    pub span: Span,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub locals: Vec<LocalId>,
    pub cleanups: Vec<CleanupId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Function,
    Block,
    UnsafeBlock,
    MatchArm,
    ForLoop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CleanupAction {
    pub span: Span,
    pub scope: ScopeId,
    pub kind: CleanupKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CleanupKind {
    Defer { expr: ExprId },
}
