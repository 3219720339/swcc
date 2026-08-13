//! 语义层：模块加载、名称解析、类型检查与 MIR 降级。

pub mod check;
pub mod mir;
pub mod symbols;
pub mod types;

pub use check::{AnalysisResult, analyze};
pub use mir::{MirExpr, MirFunction, MirModule, MirStmt, MirTarget};
pub use symbols::{FunctionSig, SymbolId, TypeTable};
pub use types::Type;
