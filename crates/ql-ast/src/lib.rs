use std::hash::{Hash, Hasher};

use ql_span::Span;

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
    pub alias_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseItem {
    pub name: String,
    pub name_span: Span,
    pub alias: Option<String>,
    pub alias_span: Option<Span>,
}

/// Qualified package or type path using `.` separators.
#[derive(Clone, Debug)]
pub struct Path {
    pub segments: Vec<String>,
    pub segment_spans: Vec<Span>,
}

impl Path {
    pub fn new(segments: Vec<String>) -> Self {
        Self {
            segment_spans: vec![Span::default(); segments.len()],
            segments,
        }
    }

    pub fn with_spans(segments: Vec<String>, segment_spans: Vec<Span>) -> Self {
        debug_assert_eq!(segments.len(), segment_spans.len());
        Self {
            segments,
            segment_spans,
        }
    }

    pub fn segment_span(&self, index: usize) -> Option<Span> {
        self.segment_spans
            .get(index)
            .copied()
            .filter(|span| !span.is_empty())
    }

    pub fn first_segment_span(&self) -> Option<Span> {
        self.segment_span(0)
    }

    pub fn last_segment_span(&self) -> Option<Span> {
        self.segment_spans
            .len()
            .checked_sub(1)
            .and_then(|index| self.segment_span(index))
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments
    }
}

impl Eq for Path {}

impl Hash for Path {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.segments.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
}

/// Top-level declarations supported by the current front-end slice.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub span: Span,
    pub kind: ItemKind,
}

impl Item {
    pub const fn new(span: Span, kind: ItemKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    Function(FunctionDecl),
    Const(GlobalDecl),
    Static(GlobalDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplBlock),
    Extend(ExtendBlock),
    TypeAlias(TypeAliasDecl),
    ExternBlock(ExternBlock),
}

/// Reusable function signature model shared by free functions, trait items, and FFI declarations.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDecl {
    pub span: Span,
    pub visibility: Visibility,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub abi: Option<String>,
    pub generics: Vec<GenericParam>,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub where_clause: Vec<WherePredicate>,
    pub body: Option<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Regular {
        name: String,
        name_span: Span,
        ty: TypeExpr,
    },
    Receiver {
        kind: ReceiverKind,
        span: Span,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverKind {
    ReadOnly,
    Mutable,
    Move,
}

/// Reusable generic parameter syntax from declaration sites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericParam {
    pub name: String,
    pub name_span: Span,
    pub bounds: Vec<Path>,
}

/// A `where` clause predicate attached to a declaration signature.
#[derive(Clone, Debug, PartialEq)]
pub struct WherePredicate {
    pub target: TypeExpr,
    pub bounds: Vec<Path>,
}

/// Shared top-level representation for `const` and `static` declarations.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalDecl {
    pub visibility: Visibility,
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDecl {
    pub visibility: Visibility,
    pub is_data: bool,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<FieldDecl>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDecl {
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
    Tuple(Vec<TypeExpr>),
    Struct(Vec<FieldDecl>),
}

/// Trait declaration surface as parsed before semantic lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct TraitDecl {
    pub visibility: Visibility,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<FunctionDecl>,
}

/// Inherent or trait implementation block.
#[derive(Clone, Debug, PartialEq)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    pub trait_ty: Option<TypeExpr>,
    pub target: TypeExpr,
    pub where_clause: Vec<WherePredicate>,
    pub methods: Vec<FunctionDecl>,
}

/// Extension method block for attaching methods outside the nominal type definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtendBlock {
    pub target: TypeExpr,
    pub methods: Vec<FunctionDecl>,
}

/// Named alias or opaque alias declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeAliasDecl {
    pub visibility: Visibility,
    pub is_opaque: bool,
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub ty: TypeExpr,
}

/// Group of imported foreign function signatures under a shared ABI.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternBlock {
    pub visibility: Visibility,
    pub abi: String,
    pub functions: Vec<FunctionDecl>,
}

/// Surface type expressions before semantic lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeExpr {
    pub span: Span,
    pub kind: TypeExprKind,
}

impl TypeExpr {
    pub const fn new(span: Span, kind: TypeExprKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeExprKind {
    Pointer {
        is_const: bool,
        inner: Box<TypeExpr>,
    },
    Array {
        element: Box<TypeExpr>,
        len: String,
    },
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
    pub span: Span,
    pub statements: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
}

/// Statement forms that appear inside blocks.
#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

impl Stmt {
    pub const fn new(span: Span, kind: StmtKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
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
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

impl Pattern {
    pub const fn new(span: Span, kind: PatternKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
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
    pub name_span: Span,
    pub pattern: Option<Box<Pattern>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureParam {
    pub name: String,
    pub span: Span,
}

/// Expression nodes preserved by the AST before HIR lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

impl Expr {
    pub const fn new(span: Span, kind: ExprKind) -> Self {
        Self { span, kind }
    }
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
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Block(Block),
    Unsafe(Block),
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
        params: Vec<ClosureParam>,
        body: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    Member {
        object: Box<Expr>,
        field: String,
        field_span: Span,
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
    Named {
        name: String,
        name_span: Span,
        value: Expr,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructLiteralField {
    pub name: String,
    pub name_span: Span,
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
    Not,
    Neg,
    Await,
    Spawn,
}

/// Parse a lexer-style integer literal into `usize` when representable.
pub fn parse_usize_literal(text: &str) -> Option<usize> {
    let normalized = text.replace('_', "");
    if let Some(rest) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        usize::from_str_radix(rest, 16).ok()
    } else if let Some(rest) = normalized
        .strip_prefix("0b")
        .or_else(|| normalized.strip_prefix("0B"))
    {
        usize::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = normalized
        .strip_prefix("0o")
        .or_else(|| normalized.strip_prefix("0O"))
    {
        usize::from_str_radix(rest, 8).ok()
    } else {
        normalized.parse::<usize>().ok()
    }
}

/// Parse a lexer-style integer literal into `i64` when representable.
pub fn parse_i64_literal(text: &str) -> Option<i64> {
    let normalized = text.replace('_', "");
    let (negative, digits) = if let Some(rest) = normalized.strip_prefix('-') {
        (true, rest)
    } else {
        (false, normalized.as_str())
    };
    let parsed = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i64::from_str_radix(rest, 16).ok()
    } else if let Some(rest) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        i64::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = digits
        .strip_prefix("0o")
        .or_else(|| digits.strip_prefix("0O"))
    {
        i64::from_str_radix(rest, 8).ok()
    } else {
        digits.parse::<i64>().ok()
    }?;

    if negative {
        parsed.checked_neg()
    } else {
        Some(parsed)
    }
}
