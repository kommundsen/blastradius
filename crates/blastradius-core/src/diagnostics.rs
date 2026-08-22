//! Diagnostics. Everything is reported with file + line (spec §6) — a
//! diagnostic without a location is a bug in this crate, tolerated only for
//! whole-file conditions (missing manifest, unreadable file).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational — e.g. a doc-glob file without frontmatter (spec §5).
    Info,
    Warning,
    /// Workspace loads but is marked invalid (spec §6).
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Workspace-relative path, forward slashes.
    pub file: String,
    /// 1-based; 0 = whole file.
    pub line: u64,
    pub message: String,
}

impl Diagnostic {
    pub fn error(file: impl Into<String>, line: u64, message: impl Into<String>) -> Self {
        Self { severity: Severity::Error, file: file.into(), line, message: message.into() }
    }
    pub fn warning(file: impl Into<String>, line: u64, message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, file: file.into(), line, message: message.into() }
    }
    pub fn info(file: impl Into<String>, line: u64, message: impl Into<String>) -> Self {
        Self { severity: Severity::Info, file: file.into(), line, message: message.into() }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "{}: {}:{}: {}", self.severity, self.file, self.line, self.message)
        } else {
            write!(f, "{}: {}: {}", self.severity, self.file, self.message)
        }
    }
}

/// True when any diagnostic is an error.
pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}
