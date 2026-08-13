//! 公共层：源码、源码范围与诊断基础类型。
//! 不依赖任何更高层组件。

pub mod diagnostics;
pub mod source;
pub mod span;

pub use diagnostics::{Diagnostic, Diagnostics, Severity};
pub use source::Source;
pub use span::Span;
