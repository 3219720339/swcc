use sw_common::Span;

use crate::token::{FloatSuffix, IntegerSuffix};

#[derive(Clone, Debug)]
pub struct Module {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: Ident,
    pub arguments: Vec<(Ident, AttributeValue)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum AttributeValue {
    Ident(Ident),
    String(String),
    Integer(i128),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub attributes: Vec<Attribute>,
    pub exported: bool,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Import(ImportDecl),
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
    TypeAlias(TypeAliasDecl),
    Variable(VariableDecl),
}

#[derive(Clone, Debug)]
pub struct ImportDecl {
    pub kind: ImportKind,
    pub path: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ImportKind {
    SideEffect,
    Named(Vec<ImportSpecifier>),
    Namespace(Ident),
}

#[derive(Clone, Debug)]
pub struct ImportSpecifier {
    pub name: Ident,
    pub alias: Option<Ident>,
}

#[derive(Clone, Debug)]
pub struct FunctionDecl {
    pub async_: bool,
    pub extern_c: bool,
    pub name: Ident,
    pub generics: Vec<Ident>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeRef>,
    pub where_clause: Vec<WhereConstraint>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub rest: bool,
    pub name: Ident,
    pub ty: TypeRef,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct WhereConstraint {
    pub name: Ident,
    pub bound: TypeRef,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StructDecl {
    pub name: Ident,
    pub generics: Vec<Ident>,
    pub where_clause: Vec<WhereConstraint>,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FieldDecl {
    pub modifiers: Vec<MemberModifier>,
    pub attributes: Vec<Attribute>,
    pub name: Ident,
    pub ty: TypeRef,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: Ident,
    pub members: Vec<EnumMember>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumMember {
    pub name: Ident,
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ClassDecl {
    pub final_: bool,
    pub name: Ident,
    pub generics: Vec<Ident>,
    pub extends: Option<TypeRef>,
    pub implements: Vec<TypeRef>,
    pub where_clause: Vec<WhereConstraint>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ClassMember {
    Field(FieldDecl),
    Constructor(ConstructorDecl),
    Destructor(DestructorDecl),
    Property(PropertyDecl),
    Method(FunctionDecl),
}

#[derive(Clone, Debug)]
pub struct ConstructorDecl {
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct DestructorDecl {
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct PropertyDecl {
    pub name: Ident,
    pub ty: TypeRef,
    pub get: Option<Block>,
    pub set: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct InterfaceDecl {
    pub name: Ident,
    pub generics: Vec<Ident>,
    pub extends: Vec<TypeRef>,
    pub where_clause: Vec<WhereConstraint>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeAliasDecl {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Let,
    Const,
}

#[derive(Clone, Debug)]
pub struct VariableDecl {
    pub kind: VarKind,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberModifier {
    Public,
    Private,
    Protected,
    Internal,
    Static,
    Virtual,
    Override,
    Final,
}

#[derive(Clone, Debug)]
pub struct TypeRef {
    pub segments: Vec<TypeSegment>,
    /// 后缀按书写顺序保存：`int[]?` 为 [Array, Nullable]。
    pub suffixes: Vec<TypeSuffix>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeSegment {
    pub name: Ident,
    pub generics: Vec<TypeRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeSuffix {
    Array,
    Nullable,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Block(Block),
    Variable(VariableDecl),
    If {
        cond: Expr,
        then: Box<Stmt>,
        else_: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<ForInit>,
        cond: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    ForEach {
        kind: VarKind,
        name: Ident,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Switch {
        value: Expr,
        cases: Vec<SwitchCase>,
        default: Option<Vec<Stmt>>,
    },
    Try {
        body: Block,
        catches: Vec<CatchClause>,
        finally: Option<Block>,
    },
    Throw(Expr),
    Defer(Expr),
    Break,
    Continue,
    Return(Option<Expr>),
    Expr(Expr),
    Empty,
}

#[derive(Clone, Debug)]
pub enum ForInit {
    Variable(VariableDecl),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct SwitchCase {
    pub value: Expr,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub struct CatchClause {
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub body: Block,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Integer {
        text: String,
        suffix: Option<IntegerSuffix>,
    },
    Float {
        text: String,
        suffix: Option<FloatSuffix>,
    },
    Str(String),
    Template(Vec<TemplatePart>),
    Char(char),
    Bool(bool),
    Null,
    Ident(Ident),
    This,
    Super,
    Group(Box<Expr>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Assign {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Conditional {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Member {
        object: Box<Expr>,
        name: Ident,
        optional: bool,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        optional: bool,
    },
    Postfix {
        expr: Box<Expr>,
        op: PostfixOp,
    },
    Array(Vec<Expr>),
    Object(Vec<ObjectField>),
    New {
        ty: TypeRef,
        args: Vec<Expr>,
    },
    Lambda {
        params: Vec<LambdaParam>,
        body: LambdaBody,
    },
}

#[derive(Clone, Debug)]
pub enum TemplatePart {
    Text(String),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct ObjectField {
    pub key: ObjectKey,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub enum ObjectKey {
    Ident(Ident),
    Str(String),
}

#[derive(Clone, Debug)]
pub struct LambdaParam {
    pub name: Ident,
    pub ty: Option<TypeRef>,
}

#[derive(Clone, Debug)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    Pos,
    BitNot,
    Inc,
    Dec,
    Await,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Coalesce,
    Or,
    And,
    BitOr,
    BitXor,
    BitAnd,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Coalesce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostfixOp {
    Inc,
    Dec,
}
