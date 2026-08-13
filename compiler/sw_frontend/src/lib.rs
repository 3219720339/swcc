//! 前端：词法分析、语法分析与抽象语法树。
//! 只判断源码结构，不处理目标平台 ABI 和机器码。

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod token;

pub use ast::Module;
pub use lexer::Lexer;
pub use parser::Parser;
pub use token::{FloatSuffix, IntegerSuffix, Keyword, Token, TokenKind};
