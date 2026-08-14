//! 语义层：模块加载、名称解析、类型检查与 MIR 降级。

pub mod check;
pub mod mir;
pub mod symbols;
pub mod types;

pub use check::{AnalysisResult, analyze};
pub use mir::{
    IterateMode, MatchArmMir, MirBinary, MirCallee, MirExpr, MirFunction, MirGlobal, MirModule,
    MirParam, MirStmt, MirStmtKind, MirTarget, MirUnary,
};
pub use symbols::{FunctionSig, SymbolId, TypeTable};
pub use types::Type;
