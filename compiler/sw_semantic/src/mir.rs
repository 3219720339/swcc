//! 最小 MIR：语义检查通过后的稳定中间表示，供 Cranelift 后端消费。

use crate::symbols::FunctionSig;
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
    /// 定义该全局的模块 id（跨模块引用时指向定义模块，codegen 按此决定
    /// Export 定义还是 Import 引用）。
    pub module: u32,
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
        /// 元素类型（浮点数组读写需按 f64 位模式转换）。
        elem: Box<Type>,
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
        /// 元素类型（浮点数组读写需按 f64 位模式转换）。
        elem: Box<Type>,
    },
    /// 数组字面量：由运行时分配并填充。
    Array {
        elem: Box<Type>,
        items: Vec<MirExpr>,
    },
    /// 数组字面量内的展开项 `...arr`（items 中与普通项混排）。
    ArraySpread(Box<MirExpr>),
    /// 可变参数打包：每个元素为 (类型标签, 元素)，用作可变参数函数的最后一个参数。
    VarArgs(Vec<(i64, MirExpr)>),
    /// 数组/字符串长度。
    Len {
        object: Box<MirExpr>,
        /// true 表示 string（len 在偏移 8），false 表示数组（len 在偏移 0）。
        string: bool,
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
    /// 三元表达式：条件分支各求值一次（与 JS 语义一致）。
    Select {
        cond: Box<MirExpr>,
        then: Box<MirExpr>,
        else_: Box<MirExpr>,
    },
    /// 赋值表达式：求值 value、写入 target，整体返回被赋的值。
    Assign {
        target: MirTarget,
        value: Box<MirExpr>,
    },
    /// 后缀 ++/--：先取旧值，写回新值，整体返回旧值。
    Postfix {
        target: MirTarget,
        op: MirUnary,
    },
    /// 创建闭包对象：运行时分配 { fn 指针, 环境槽数组 }。
    ClosureNew {
        name: String,
        captures: Vec<MirExpr>,
        sig: FunctionSig,
    },
    /// 隐藏函数内部读取捕获槽。
    EnvGet {
        slot: usize,
    },
    /// 创建类对象并调用构造函数。
    New {
        class: u32,
        /// 构造函数签名（跨模块 new 时用于导入基类/他模块构造函数）。
        sig: FunctionSig,
        args: Vec<MirExpr>,
    },
    /// ADT 枚举构造：运行时对象布局 tag@0、payload 字段 @8*(i+1)。
    EnumNew {
        tag: i64,
        fields: Vec<MirExpr>,
    },
    /// 读取枚举对象的 tag。
    EnumTag {
        object: Box<MirExpr>,
    },
    /// 读取枚举对象的第 index 个 payload 字段。
    EnumField {
        object: Box<MirExpr>,
        index: usize,
        /// 字段类型（浮点字段按 f64 位模式存取）。
        elem: Type,
    },
    /// `expr?` 错误传播：Err 时构造并返回函数 Result 的 Err，Ok 时取出 payload。
    TryPropagate {
        object: Box<MirExpr>,
        err_tag: i64,
        ret_err_tag: i64,
        elem: Type,
    },
    /// 数组 map：内联循环，逐元素调闭包生成新数组。
    ArrayMap {
        object: Box<MirExpr>,
        closure: Box<MirExpr>,
        sig: FunctionSig,
        elem: Type,
        ret_elem: Type,
    },
    /// 数组 filter：内联循环，保留闭包返回 true 的元素。
    ArrayFilter {
        object: Box<MirExpr>,
        closure: Box<MirExpr>,
        sig: FunctionSig,
        elem: Type,
    },
    /// 数组迭代（forEach/some/every/find）：内联循环调闭包。
    ArrayIterate {
        object: Box<MirExpr>,
        closure: Box<MirExpr>,
        sig: FunctionSig,
        elem: Type,
        mode: IterateMode,
    },
    /// match 表达式：按 tag 分派，每分支解构绑定后求值，结果进公共槽。
    MatchExpr {
        value: Box<MirExpr>,
        arms: Vec<MatchArmMir>,
        ret: Type,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterateMode {
    ForEach,
    Some,
    Every,
    Find,
}

#[derive(Clone, Debug)]
pub struct MatchArmMir {
    /// 匹配的变体 tag；None 表示通配 `_`（兜底分支）。
    pub tag: Option<i64>,
    /// 解构绑定：(局部变量 index, 字段类型)。
    pub bindings: Vec<(usize, Type)>,
    pub body: MirExpr,
}

#[derive(Clone, Debug)]
pub enum MirCallee {
    Function {
        module: u32,
        name: String,
        sig: FunctionSig,
    },
    Method {
        class: u32,
        name: String,
        sig: FunctionSig,
    },
    /// 接口方法调用：经 vtable 间接派发（args[0] 为接收者对象）。
    InterfaceMethod {
        interface: u32,
        index: usize,
        sig: FunctionSig,
    },
    /// 通过闭包对象间接调用：args[0] 是闭包指针。
    Closure {
        sig: FunctionSig,
    },
    Extern {
        name: String,
        sig: FunctionSig,
    },
    Intrinsic {
        name: String,
    },
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
