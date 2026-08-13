use crate::span::Span;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub file: Option<PathBuf>,
}

/// 诊断集合：一次编译输出全部可发现的错误。
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn error(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.error_at(message, span, None);
    }

    pub fn error_at(
        &mut self,
        message: impl Into<String>,
        span: Option<Span>,
        file: Option<PathBuf>,
    ) {
        self.items.push(Diagnostic {
            severity: Severity::Error,
            message: message.into(),
            span,
            file,
        });
    }

    pub fn warning(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.items.push(Diagnostic {
            severity: Severity::Warning,
            message: message.into(),
            span,
            file: None,
        });
    }

    pub fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.severity == Severity::Error)
    }

    pub fn extend(&mut self, other: Diagnostics) {
        self.items.extend(other.items);
    }
}
