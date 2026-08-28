//! Diagnostic types for the Flux type checker.
//!
//! Every error carries a [`Span`] (the "where"), a `message` (the "what"), an
//! optional `hint` (the "how"), and - when the failure is a type mismatch - the
//! `expected` and `actual` types (the "why"). This follows the diagnostic
//! contract in `AGENTS.md` S3.11.
//!
//! On top of the per-phase errors ([`TypeError`] and the parse / lowering /
//! runtime faults), this module defines the unified [`FluxError`] umbrella: the
//! single shape the dev server and host runtimes emit, always with a
//! `what`/`where`/`why`/`how` payload. `LoweringError` lives in `flux-ir`, which
//! depends on `flux-types`, so it is represented via [`FluxError::Compile`] with
//! [`CompilePhase::Lower`] (built through [`CompileError::from_lowering`]) rather
//! than a `From` impl that would form a dependency cycle.

use flux_parser::ParseError;
use flux_syntax::{Span, TypeKind};
use flux_vm_ref::{VmError, VmErrorKind};
use std::fmt;

/// A type-checking failure.
///
/// Construct one with [`TypeError::mismatch`] / [`TypeError::new`] and render it
/// for a human with [`TypeError::render`], which needs the original source text
/// to turn the [`Span`] offsets into `file:line:col`.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeError {
    /// What went wrong, in prose (e.g. "type mismatch in `Counter`").
    pub message: String,
    /// Why it went wrong and how to fix it, when known.
    pub hint: Option<String>,
    /// Where it went wrong, as raw byte offsets into the source file.
    pub span: Span,
    /// The type that was expected at this site, when applicable.
    pub expected: Option<Box<TypeKind>>,
    /// The type that was actually found, when applicable.
    pub actual: Option<Box<TypeKind>>,
}

impl TypeError {
    /// Builds a diagnostic with only a message and span.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            hint: None,
            span,
            expected: None,
            actual: None,
        }
    }

    /// Builds a type-mismatch diagnostic, capturing expected/actual.
    #[must_use]
    pub fn mismatch(expected: &crate::TcType, actual: &crate::TcType, span: Span) -> Self {
        let message = format!("expected `{expected}`, got `{actual}`");
        Self {
            message,
            hint: Some(
                "check that the expression on the right has the type declared \
                 on the left, or adjust the annotation"
                    .to_owned(),
            ),
            span,
            expected: Some(Box::new(expected.to_typekind())),
            actual: Some(Box::new(actual.to_typekind())),
        }
    }

    /// Attaches a "how" hint, returning `self` for chaining.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Renders the diagnostic as a Rust-style `file:line:col` message.
    ///
    /// `source` must be the text that was parsed to produce [`Span`]; line and
    /// column are computed from the span's start offset. The `path` is shown in
    /// the gutter.
    #[must_use]
    pub fn render(&self, source: &str, path: &str) -> String {
        let (line, col) = line_col(source, self.span.start as usize);
        let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
        let mut out = String::new();
        out.push_str(&format!("error: {}\n", self.message));
        out.push_str(&format!("  --> {path}:{line}:{col}\n"));
        out.push_str("   |\n");
        out.push_str(&format!("{} | {}\n", line, line_text));
        out.push_str(&format!(
            "   | {}{}\n",
            " ".repeat(col.saturating_sub(1)),
            "^".repeat(usize::max(1, (self.span.end - self.span.start) as usize))
        ));
        if let Some(hint) = &self.hint {
            out.push_str(&format!("   |\n   = hint: {hint}\n"));
        }
        out
    }
}

/// Converts a byte `offset` into 1-based `(line, column)` within `source`.
#[must_use]
pub(crate) fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}

impl std::error::Error for TypeError {}

/// The capability a `CALL_CAP` reached for but was denied (red banner, not crash).
///
/// A denied OS permission (or an unknown capability id) surfaces as this variant
/// of [`FluxError`]; it never reaches native code and never panics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityError {
    /// Numeric capability id that was invoked.
    pub cap_id: u32,
    /// Human capability name, when one is registered.
    pub cap_name: Option<String>,
    /// Numeric method id that was invoked.
    pub method_id: u16,
    /// Human method name, when one is registered.
    pub method_name: Option<String>,
    /// The OS permission token that was required (e.g. `.camera`).
    pub required_permission: String,
    /// The human-readable reason the grant was denied.
    pub why: String,
}

/// A compile-phase failure (parse / type-check / lowering) that carries a source span.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    /// What went wrong, in prose.
    pub message: String,
    /// Where it went wrong.
    pub span: Span,
    /// How to fix it, when known.
    pub hint: Option<String>,
    /// Which compile phase produced the error.
    pub phase: CompilePhase,
}

impl CompileError {
    /// Builds a [`CompileError`] from a `LoweringError` (which lives in `flux-ir`
    /// and cannot be converted via `From` without a dependency cycle).
    #[must_use]
    pub fn from_lowering(message: String, span: Span) -> Self {
        Self {
            message,
            span,
            hint: Some(
                "the typed AST could not be lowered; the construct may not yet be \
                 supported by the MLP lowering pass"
                    .to_owned(),
            ),
            phase: CompilePhase::Lower,
        }
    }
}

/// Which front-end phase an error originated from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilePhase {
    /// Lexing / parsing.
    Parse,
    /// Type checking.
    Type,
    /// Lowering to the reactive IR.
    Lower,
}

/// A runtime VM fault, classified by its stable [`VmErrorKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    /// The category of fault (must match the ISA vector contract).
    pub kind: VmErrorKind,
    /// Byte offset of the offending instruction, when available.
    pub offset: u32,
    /// Source span, when the handler was lowered from `.flux`.
    pub span: Option<Span>,
}

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

/// Builds a denied-capability [`FluxError`] from the capability/method ids and
/// the permission token that was missing.
///
/// `cap_id` / `method_id` are the raw `CALL_CAP` operands; the IDL names are
/// resolved by the caller (or left `None`) so the host can display a precise
/// red banner.
#[must_use]
pub fn capability_denied(
    cap_id: u32,
    method_id: u16,
    cap_name: Option<String>,
    method_name: Option<String>,
    required_permission: String,
) -> FluxError {
    let why = format!("required permission `{required_permission}` was not granted");
    FluxError::Capability(CapabilityError {
        why,
        cap_id,
        cap_name,
        method_id,
        method_name,
        required_permission,
    })
}

/// Builds a [`FluxError::Compile`] carrying message, span, optional hint and phase.
#[must_use]
pub fn compile_error(
    message: impl Into<String>,
    span: Span,
    hint: Option<String>,
    phase: CompilePhase,
) -> FluxError {
    FluxError::Compile(CompileError {
        message: message.into(),
        span,
        hint,
        phase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::TcType;

    #[test]
    fn mismatch_render_contains_location_and_hint() {
        let span = flux_syntax::Span::new(0, 10, 14);
        let err = TypeError::mismatch(&TcType::Int, &TcType::String, span);
        let rendered = err.render("compo X\n  count = \"nope\"\n", "main.flux");
        assert!(rendered.contains("main.flux:2:3"), "got: {rendered}");
        assert!(rendered.contains("hint:"), "got: {rendered}");
        assert!(rendered.contains("expected `Int`"), "got: {rendered}");
    }

    #[test]
    fn new_error_without_hint_renders_only_what_where() {
        let span = flux_syntax::Span::new(0, 0, 4);
        let err = TypeError::new("cannot lower construct", span);
        let rendered = err.render("compo X", "x.flux");
        assert!(rendered.contains("x.flux:1:1"));
        assert!(!rendered.contains("hint:"));
    }

    #[test]
    fn flux_error_class_and_accessors() {
        let denied = capability_denied(
            1,
            1,
            Some("Camera".to_owned()),
            Some("take".to_owned()),
            ".camera".to_owned(),
        );
        assert_eq!(denied.class(), "capability");
        assert!(denied.what().contains("Camera"));
        assert!(denied.how().unwrap().contains(".camera"));
        assert!(denied.where_span().is_none());
    }
}
