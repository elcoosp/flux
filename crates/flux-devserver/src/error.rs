//! Dev-server error types (FLUX-019).
//!
//! Compilation failures are *not* errors of the server: they are reported to the
//! host as an `Error` frame ([`Diagnostic`]) while the previous good tree stays
//! live. [`DevServerError`] is reserved for failures that stop the server
//! itself, such as a port that cannot be bound.
//!
//! A pipeline failure may originate as any of the per-crate error types
//! (`TypeError`, `ParseError`, `LoweringError`, `VmError`); [`Diagnostic`]
//! converts from any of them so the server emits one diagnostic shape to the
//! host, always with a `what`/`where`/`why`/`how` payload (AGENTS.md §3.11).

use flux_syntax::Span;
use thiserror::Error;

/// A failure that prevents the dev server from starting or serving.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DevServerError {
    /// A listening socket could not be bound.
    #[error(
        "cannot bind {kind} listener on {addr}: {source} — hint: another dev server may already be running; pass a different address"
    )]
    Bind {
        /// Which listener failed (`"websocket"` or `"http"`).
        kind: &'static str,
        /// The address the bind was attempted on.
        addr: std::net::SocketAddr,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// The project root could not be watched.
    #[error(
        "cannot watch project root {root}: {message} — hint: check the path exists and is readable"
    )]
    Watch {
        /// The root that could not be watched.
        root: String,
        /// The underlying watcher error.
        message: String,
    },
}

/// A source-level diagnostic produced by the compile pipeline.
///
/// Carries the message, an optional hint, and the [`Span`] the failure points
/// at (AGENTS.md §3.7: what / where / why / how). It is shipped to the host as
/// an `Error` frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// What went wrong, including how to fix it when known.
    pub message: String,
    /// Where it went wrong, when the phase reported a span.
    pub span: Option<Span>,
}

impl Diagnostic {
    /// Builds a diagnostic from a message and an optional span.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.span {
            Some(span) => write!(
                f,
                "{} (file {} bytes {}..{})",
                self.message, span.file_id, span.start, span.end
            ),
            None => write!(f, "{}", self.message),
        }
    }
}

impl From<flux_types::FluxError> for Diagnostic {
    fn from(err: flux_types::FluxError) -> Self {
        let mut message = format!("[{}] {}", err.class(), err.what());
        if let Some(how) = err.how() {
            message.push_str(&format!(" — how: {how}"));
        }
        Diagnostic {
            message,
            span: err.where_span(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_syntax::Span;

    #[test]
    fn diagnostic_carries_message_and_span() {
        // A diagnostic must round-trip its what/where so the host's red banner
        // can point at the offending source without re-deriving it.
        let diag = Diagnostic::new("type mismatch", Some(Span::new(2, 4, 9)));
        assert_eq!(diag.span, Some(Span::new(2, 4, 9)));
        assert!(diag.message.contains("type mismatch"));
        assert_eq!(
            diag.to_string(),
            "type mismatch (file 2 bytes 4..9)",
            "display must include the span location"
        );
    }
}
