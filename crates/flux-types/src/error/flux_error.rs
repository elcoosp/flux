use super::capability_error::CapabilityError;
use super::compile_err::{CompileError, CompilePhase};
use super::runtime_error::RuntimeError;
use super::type_error::TypeError;
use flux_parser::ParseError;
use flux_syntax::Span;
use flux_vm_ref::VmError;
use std::fmt;
/// The unified umbrella error for the front-end pipeline (LANE-I, FLUX-02X).
///
/// Every per-phase error (`TypeError`, parse, lower, VM) collapses into
/// [`FluxError`] so the dev server and host runtimes can render one diagnostic
/// shape - always with a `what`/`where`/`why`/`how` payload (`AGENTS.md` S3.11).
///
/// The VM's [`VmErrorKind`] discriminants are load-bearing (ISA vectors assert
/// them), so [`FluxError::Runtime`] wraps the existing [`VmErrorKind`] rather
/// than re-defining new fault codes - this enum is purely a *classification*
/// layer on top of the stable per-crate types.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FluxError {
    /// A `CALL_CAP` was gated but the required OS permission was not granted.
    Capability(CapabilityError),
    /// A compile-phase failure with a source span.
    Compile(CompileError),
    /// A runtime fault surfaced from the VM (e.g. div-by-zero, null deref).
    Runtime(RuntimeError),
}

impl FluxError {
    /// Returns the short classification label (`"compile"` / `"runtime"` /
    /// `"capability"`).
    #[must_use]
    pub fn class(&self) -> &'static str {
        match self {
            Self::Compile(_) => "compile",
            Self::Runtime(_) => "runtime",
            Self::Capability(_) => "capability",
        }
    }

    /// Returns the "what" of the error - a concise prose description.
    #[must_use]
    pub fn what(&self) -> String {
        match self {
            Self::Compile(e) => e.message.clone(),
            Self::Runtime(e) => e.kind.to_string(),
            Self::Capability(e) => format!(
                "capability `{}.{}` denied: {}",
                e.cap_name.as_deref().unwrap_or("?"),
                e.method_name.as_deref().unwrap_or("?"),
                e.why
            ),
        }
    }

    /// Returns the "where" - the [`Span`] the error points at, if any.
    #[must_use]
    pub fn where_span(&self) -> Option<Span> {
        match self {
            Self::Compile(e) => Some(e.span),
            Self::Runtime(e) => e.span,
            Self::Capability(_) => None,
        }
    }

    /// Returns the "why" - the reason the failure occurred.
    #[must_use]
    pub fn why(&self) -> String {
        match self {
            Self::Compile(e) => e
                .hint
                .clone()
                .unwrap_or_else(|| "see the source location for context".to_owned()),
            Self::Runtime(e) => format!("VM fault at byte offset {}", e.offset),
            Self::Capability(e) => format!(
                "the OS grant `{}` was not authorized for this capability call",
                e.required_permission
            ),
        }
    }

    /// Returns the "how" - the actionable remediation, if known.
    #[must_use]
    pub fn how(&self) -> Option<String> {
        match self {
            Self::Compile(e) => e.hint.clone(),
            Self::Runtime(_) => Some(
                "check the handler bytecode for the offending instruction; the fault is \
                 deterministic and reproducible"
                    .to_owned(),
            ),
            Self::Capability(e) => Some(format!(
                "request and await the `{}` permission before calling this capability (the \
                 host surfaces a system prompt for the user to grant it)",
                e.required_permission
            )),
        }
    }
}

impl fmt::Display for FluxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(e) => {
                write!(f, "[compile:{:?}] {}", e.phase, e.message)?;
                if let Some(hint) = &e.hint {
                    write!(f, " (hint: {hint})")?;
                }
                Ok(())
            }
            Self::Runtime(e) => write!(f, "[runtime] {} at offset {}", e.kind, e.offset),
            Self::Capability(e) => write!(
                f,
                "[capability] `{}.{}` ({}/{}): {}",
                e.cap_name.as_deref().unwrap_or("?"),
                e.method_name.as_deref().unwrap_or("?"),
                e.cap_id,
                e.method_id,
                e.why
            ),
        }
    }
}

impl std::error::Error for FluxError {}

impl From<TypeError> for FluxError {
    fn from(e: TypeError) -> Self {
        Self::Compile(CompileError {
            message: e.message,
            span: e.span,
            hint: e.hint,
            phase: CompilePhase::Type,
        })
    }
}

impl From<ParseError> for FluxError {
    fn from(e: ParseError) -> Self {
        Self::Compile(CompileError {
            message: e.message,
            span: e.span,
            hint: e.hint,
            phase: CompilePhase::Parse,
        })
    }
}

impl From<VmError> for FluxError {
    fn from(e: VmError) -> Self {
        Self::Runtime(RuntimeError {
            kind: e.kind,
            offset: e.offset,
            span: e.span,
        })
    }
}
