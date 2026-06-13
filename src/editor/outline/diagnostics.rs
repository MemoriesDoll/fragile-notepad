use super::types::{OutlineDiagnostic, OutlineDiagnosticSeverity};

pub(super) fn info(message: impl Into<String>) -> OutlineDiagnostic {
    diagnostic(OutlineDiagnosticSeverity::Info, message)
}

pub(super) fn warning(message: impl Into<String>) -> OutlineDiagnostic {
    diagnostic(OutlineDiagnosticSeverity::Warning, message)
}

pub(super) fn error(message: impl Into<String>) -> OutlineDiagnostic {
    diagnostic(OutlineDiagnosticSeverity::Error, message)
}

fn diagnostic(
    severity: OutlineDiagnosticSeverity,
    message: impl Into<String>,
) -> OutlineDiagnostic {
    OutlineDiagnostic::new(severity, message, None)
}
