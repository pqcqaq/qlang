mod ids;
mod lower;

use ql_ast::{BinaryOp, PackageDecl, Path, ReceiverKind, UnaryOp, UseDecl, Visibility};
use ql_span::Span;

pub use ids::{BlockId, ExprId, ItemId, LocalId, PatternId, StmtId, TypeId};
pub use lower::lower_module;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FunctionRef {
    Item(ItemId),
    ExternBlockMember { block: ItemId, index: usize },
}

/// Semantic-ready module lowered out of the surface AST.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub package: Option<PackageDecl>,
    pub uses: Vec<UseDecl>,
    pub items: Vec<ItemId>,
    item_data: Vec<Item>,
    type_data: Vec<Type>,
    block_data: Vec<Block>,
    stmt_data: Vec<Stmt>,
    pattern_data: Vec<Pattern>,
    expr_data: Vec<Expr>,
    local_data: Vec<Local>,
}

impl Module {
    pub fn item(&self, id: ItemId) -> &Item {
        &self.item_data[id.index()]
    }

    pub fn ty(&self, id: TypeId) -> &Type {
        &self.type_data[id.index()]
    }

    pub fn block(&self, id: BlockId) -> &Block {
        &self.block_data[id.index()]
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmt_data[id.index()]
    }

    pub fn pattern(&self, id: PatternId) -> &Pattern {
        &self.pattern_data[id.index()]
    }

    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.expr_data[id.index()]
    }

    pub fn local(&self, id: LocalId) -> &Local {
        &self.local_data[id.index()]
    }

    pub fn function(&self, function_ref: FunctionRef) -> &Function {
        match function_ref {
            FunctionRef::Item(item_id) => match &self.item(item_id).kind {
                ItemKind::Function(function) => function,
                _ => panic!("HIR function reference must point at a top-level function item"),
            },
            FunctionRef::ExternBlockMember { block, index } => match &self.item(block).kind {
                ItemKind::ExternBlock(extern_block) => extern_block
                    .functions
                    .get(index)
                    .expect("HIR extern function reference index must be valid"),
                _ => panic!("HIR extern function reference must point at an extern block item"),
            },
        }
    }

    pub const fn function_owner_item(&self, function_ref: FunctionRef) -> ItemId {
        match function_ref {
            FunctionRef::Item(item_id) | FunctionRef::ExternBlockMember { block: item_id, .. } => {
                item_id
            }
        }
    }

    pub fn locals(&self) -> &[Local] {
        &self.local_data
    }

    pub(crate) fn alloc_item(&mut self, item: Item) -> ItemId {
        let id = ItemId::from_index(self.item_data.len());
        self.item_data.push(item);
        id
    }

    pub(crate) fn alloc_type(&mut self, ty: Type) -> TypeId {
        let id = TypeId::from_index(self.type_data.len());
        self.type_data.push(ty);
        id
    }

    pub(crate) fn alloc_block(&mut self, block: Block) -> BlockId {
        let id = BlockId::from_index(self.block_data.len());
        self.block_data.push(block);
        id
    }

    pub(crate) fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = StmtId::from_index(self.stmt_data.len());
        self.stmt_data.push(stmt);
        id
    }

    pub(crate) fn alloc_pattern(&mut self, pattern: Pattern) -> PatternId {
        let id = PatternId::from_index(self.pattern_data.len());
        self.pattern_data.push(pattern);
        id
    }

    pub(crate) fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId::from_index(self.expr_data.len());
        self.expr_data.push(expr);
        id
    }

    pub(crate) fn alloc_local(&mut self, local: Local) -> LocalId {
        let id = LocalId::from_index(self.local_data.len());
        self.local_data.push(local);
        id
    }
}

/// Top-level definition stored in the HIR item arena.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub span: Span,
    pub kind: ItemKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    Function(Function),
    Const(Global),
    Static(Global),
    Struct(Struct),
    Enum(Enum),
    Trait(Trait),
    Impl(Impl),
    Extend(Extend),
    TypeAlias(TypeAlias),
    ExternBlock(ExternBlock),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub span: Span,
    pub visibility: Visibility,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub abi: Option<String>,
    pub generics: Vec<GenericParam>,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub return_type: Option<TypeId>,
    pub where_clause: Vec<WherePredicate>,
    pub body: Option<BlockId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Regular(RegularParam),
    Receiver(ReceiverParam),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegularParam {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverParam {
    pub kind: ReceiverKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericParam {
    pub name: String,
    pub name_span: Span,
    pub bounds: Vec<Path>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WherePredicate {
    pub target: TypeId,
    pub bounds: Vec<Path>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Global {
    pub span: Span,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: Span,
    pub ty: TypeId,
    pub value: ExprId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Struct {
    pub span: Span,
    pub visibility: Visibility,
    pub is_data: bool,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeId,
    pub default: Option<ExprId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Enum {
    pub span: Span,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub name_span: Span,
    pub fields: VariantFields,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VariantFields {
    Unit,
    Tuple(Vec<TypeId>),
    Struct(Vec<Field>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trait {
    pub span: Span,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Impl {
    pub span: Span,
    pub generics: Vec<GenericParam>,
    pub trait_ty: Option<TypeId>,
    pub target: TypeId,
    pub where_clause: Vec<WherePredicate>,
    pub methods: Vec<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Extend {
    pub span: Span,
    pub target: TypeId,
    pub methods: Vec<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeAlias {
    pub span: Span,
    pub visibility: Visibility,
    pub is_opaque: bool,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub ty: TypeId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternBlock {
    pub span: Span,
    pub visibility: Visibility,
    pub abi: String,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    pub span: Span,
    pub kind: TypeKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    Pointer { is_const: bool, inner: TypeId },
    Named { path: Path, args: Vec<TypeId> },
    Tuple(Vec<TypeId>),
    Callable { params: Vec<TypeId>, ret: TypeId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub span: Span,
    pub statements: Vec<StmtId>,
    pub tail: Option<ExprId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Let {
        mutable: bool,
        pattern: PatternId,
        value: ExprId,
    },
    Return(Option<ExprId>),
    Defer(ExprId),
    Break,
    Continue,
    While {
        condition: ExprId,
        body: BlockId,
    },
    Loop {
        body: BlockId,
    },
    For {
        is_await: bool,
        pattern: PatternId,
        iterable: ExprId,
        body: BlockId,
    },
    Expr {
        expr: ExprId,
        terminated: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Binding(LocalId),
    Tuple(Vec<PatternId>),
    Path(Path),
    TupleStruct {
        path: Path,
        items: Vec<PatternId>,
    },
    Struct {
        path: Path,
        fields: Vec<PatternField>,
        has_rest: bool,
    },
    Integer(String),
    String(String),
    Bool(bool),
    NoneLiteral,
    Wildcard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternField {
    pub name: String,
    pub name_span: Span,
    pub pattern: PatternId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Name(String),
    Integer(String),
    String {
        value: String,
        is_format: bool,
    },
    Bool(bool),
    NoneLiteral,
    Tuple(Vec<ExprId>),
    Array(Vec<ExprId>),
    Block(BlockId),
    Unsafe(BlockId),
    If {
        condition: ExprId,
        then_branch: BlockId,
        else_branch: Option<ExprId>,
    },
    Match {
        value: ExprId,
        arms: Vec<MatchArm>,
    },
    Closure {
        is_move: bool,
        params: Vec<LocalId>,
        body: ExprId,
    },
    Call {
        callee: ExprId,
        args: Vec<CallArg>,
    },
    Member {
        object: ExprId,
        field: String,
    },
    Bracket {
        target: ExprId,
        items: Vec<ExprId>,
    },
    StructLiteral {
        path: Path,
        fields: Vec<StructLiteralField>,
    },
    Binary {
        left: ExprId,
        op: BinaryOp,
        right: ExprId,
    },
    Unary {
        op: UnaryOp,
        expr: ExprId,
    },
    Question(ExprId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: PatternId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallArg {
    Positional(ExprId),
    Named {
        name: String,
        name_span: Span,
        value: ExprId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructLiteralField {
    pub name: String,
    pub name_span: Span,
    pub value: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Local {
    pub name: String,
    pub span: Span,
}
