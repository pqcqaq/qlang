/// Parsed source file after syntactic analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub package: Option<PackageDecl>,
    pub uses: Vec<UseDecl>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageDecl {
    pub path: Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseDecl {
    pub prefix: Path,
    pub group: Option<Vec<UseItem>>,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseItem {
    pub name: String,
    pub alias: Option<String>,
}

/// Qualified package or type path using `.` separators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<String>,
}

impl Path {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
}

/// Top-level declarations supported by the current front-end slice.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Impl(ImplBlock),
}

/// Function signature plus body as preserved by the parser.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDecl {
    pub visibility: Visibility,
    pub is_async: bool,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Regular { name: String, ty: TypeExpr },
    Receiver(ReceiverKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverKind {
    ReadOnly,
    Mutable,
    Move,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDecl {
    pub visibility: Visibility,
    pub is_data: bool,
    pub name: String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDecl {
    pub visibility: Visibility,
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: VariantFields,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VariantFields {
    Unit,
    Tuple(Vec<TypeExpr>),
    Struct(Vec<FieldDecl>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImplBlock {
    pub target: Path,
    pub methods: Vec<FunctionDecl>,
}

/// Surface type expressions before semantic lowering.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeExpr {
    Named {
        path: Path,
        args: Vec<TypeExpr>,
    },
    Tuple(Vec<TypeExpr>),
    Callable {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
    },
}

/// A lexical block with ordered statements and an optional tail expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
}

/// Statement forms that appear inside blocks.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Let {
        mutable: bool,
        pattern: Pattern,
        value: Expr,
    },
    Return(Option<Expr>),
    Defer(Expr),
    Break,
    Continue,
    While {
        condition: Expr,
        body: Block,
    },
    Loop {
        body: Block,
    },
    For {
        is_await: bool,
        pattern: Pattern,
        iterable: Expr,
        body: Block,
    },
    Expr {
        expr: Expr,
        terminated: bool,
    },
}

/// Patterns used by bindings and match arms in the current parser slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    Name(String),
    Tuple(Vec<Pattern>),
    Path(Path),
    TupleStruct {
        path: Path,
        items: Vec<Pattern>,
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
    pub pattern: Option<Box<Pattern>>,
}

/// Expression nodes preserved by the AST before HIR lowering.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Name(String),
    Integer(String),
    String {
        value: String,
        is_format: bool,
    },
    Bool(bool),
    NoneLiteral,
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Block(Block),
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Box<Expr>>,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Closure {
        is_move: bool,
        params: Vec<String>,
        body: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    Member {
        object: Box<Expr>,
        field: String,
    },
    Bracket {
        target: Box<Expr>,
        items: Vec<Expr>,
    },
    StructLiteral {
        path: Path,
        fields: Vec<StructLiteralField>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Question(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named { name: String, value: Expr },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructLiteralField {
    pub name: String,
    pub value: Option<Expr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    EqEq,
    BangEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Await,
    Spawn,
}
