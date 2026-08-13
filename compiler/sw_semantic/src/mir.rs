//! 最小 MIR：语义检查通过后的稳定中间表示，供 Cranelift 后端消费。

use crate::types::Type;

#[derive(Clone, Debug)]
pub struct MirModule {
    pub module_id: u32,
    pub functions: Vec<MirFunction>,
    pub globals: Vec<MirGlobal>,
    /// 字符串字面量池（去重后）。
    pub strings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct MirGlobal {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub init: Option<MirExpr>,
}

#[derive(Clone, Debug)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<MirParam>,
    pub ret: Type,
    pub locals: Vec<MirLocal>,
    pub body: Vec<MirStmt>,
    pub extern_c: bool,
}

#[derive(Clone, Debug)]
pub struct MirParam {
    pub name: String,
    pub ty: Type,
}

#[derive(Clone, Debug)]
pub struct MirLocal {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Clone, Debug)]
pub struct MirStmt {
    pub kind: MirStmtKind,
}

impl MirStmt {
    pub fn new(kind: MirStmtKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone, Debug)]
pub enum MirStmtKind {
    /// 局部变量声明；`init` 缺失时由后端零初始化。
    VarDecl {
        local: usize,
        init: Option<MirExpr>,
    },
    Assign {
        target: MirTarget,
        value: MirExpr,
    },
    If {
        cond: MirExpr,
        then: Vec<MirStmt>,
        else_: Vec<MirStmt>,
    },
    While {
        cond: MirExpr,
        body: Vec<MirStmt>,
    },
    Return(Option<MirExpr>),
    Expr(MirExpr),
    Break,
    Continue,
}

#[derive(Clone, Debug)]
pub enum MirTarget {
    Local(usize),
    Global(u32),
    Field {
        object: Box<MirExpr>,
        /// 布局中的字段序号（类字段为基类优先的展平序号）。
        index: usize,
    },
    Index {
        object: Box<MirExpr>,
        index: Box<MirExpr>,
    },
}

#[derive(Clone, Debug)]
pub enum MirExpr {
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(usize),
    Null,
    Local(usize),
    Global(u32),
    Unary {
        op: MirUnary,
        expr: Box<MirExpr>,
    },
    Binary {
        op: MirBinary,
        left: Box<MirExpr>,
        right: Box<MirExpr>,
    },
    Call {
        callee: MirCallee,
        args: Vec<MirExpr>,
    },
    Field {
        object: Box<MirExpr>,
        index: usize,
    },
    Index {
        object: Box<MirExpr>,
        index: Box<MirExpr>,
    },
    /// 数组字面量：由运行时分配并填充。
    Array {
        elem: Box<Type>,
        items: Vec<MirExpr>,
    },
    /// 数组/字符串长度。
    Len {
        object: Box<MirExpr>,
    },
    /// 结构体字面量（值类型）。
    Struct {
        ty: Type,
        fields: Vec<(usize, MirExpr)>,
    },
    /// 显式转换。
    Cast {
        expr: Box<MirExpr>,
        to: Type,
    },
    /// 创建类对象并调用构造函数。
    New {
        class: u32,
        args: Vec<MirExpr>,
    },
}

#[derive(Clone, Debug)]
pub enum MirCallee {
    Function { module: u32, name: String },
    Method { class: u32, method: usize },
    Extern { name: String },
    Intrinsic { name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirUnary {
    Neg,
    Not,
    BitNot,
    Pos,
    Inc,
    Dec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirBinary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Coalesce,
}
