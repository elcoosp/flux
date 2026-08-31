use flux_syntax::Span;
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
