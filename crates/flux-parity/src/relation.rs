//! The parity relation: reduce all three paths to [`ViewNode`] trees and assert
//! they are structurally equivalent, snapshotting the result with `insta`.
//!
//! For each Appendix B.3 example we build:
//! 1. the **dev** tree — [`crate::model::from_ast`] over the parsed surface AST
//!    (the authoritative "what the user wrote", and exactly what codegen derives
//!    from);
//! 2. the **swift** tree — [`crate::recognize_swift::recognize`] over the emitted
//!    SwiftUI source;
//! 3. the **kotlin** tree — [`crate::recognize_kotlin::recognize`] over the
//!    emitted Compose source.
//!
//! The equivalence relation is *structural identity* of these three trees. Because
//! the ForEach body is intentionally empty in all three paths (keyed items are
//! reconciled at runtime by the host, FLUX-014), an empty `ForEach` body is the
//! expected, faithful shape and is asserted to match — never flagged as a
//! divergence.
//!
//! A lowering failure is a **hard error**, not a silently-okayed "unsupported"
//! result: every Appendix B.3 example must ship with a lowering pass that the
//! release codegen backends can exercise, so `check_parity` returns
//! [`ParityPipelineError`] instead of swallowing it. This is what makes CI fail
//! loudly when a new feature forgets its lowering pass (issue 5).

use flux_ir::LoweredIr;
use flux_parser::Ast;

use flux_types::TypedAST;

use crate::equivalence::structurally_equal;
use crate::model::ViewNode;
use crate::recognize_kotlin::recognize as recognize_kotlin;
use crate::recognize_swift::recognize as recognize_swift;

/// A full parity result for one example: the three reduced structural trees.
#[derive(Clone, Debug)]
pub struct ParityReport {
    /// The dev-path (surface AST) structural tree.
    pub dev: Vec<ViewNode>,
    /// The Swift release-path structural tree.
    pub swift: Vec<ViewNode>,
    /// The Kotlin release-path structural tree.
    pub kotlin: Vec<ViewNode>,
}

impl ParityReport {
    /// Returns `true` when the dev tree equals both release trees.
    ///
    /// This is the core parity contract: what the user wrote (dev) must be
    /// structurally identical to what the Swift and Kotlin backends generate.
    #[must_use]
    pub fn is_equivalent(&self) -> bool {
        structurally_equal(&self.dev, &self.swift) && structurally_equal(&self.swift, &self.kotlin)
    }

    /// A one-line verdict used in snapshot output.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        if self.is_equivalent() {
            "equivalent"
        } else {
            "DIVERGENT"
        }
    }
}

/// An error produced while running the dev→release parity pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityPipelineError(pub String);

impl std::fmt::Display for ParityPipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parity pipeline error: {}", self.0)
    }
}

impl std::error::Error for ParityPipelineError {}

/// Compiles a `.flux` source through the full pipeline: parse → type-check →
/// lower.
///
/// The lowered IR is required by the release codegen backends. A lowering
/// failure is a **hard error** (not a silently-okayed "unsupported" result):
/// every B.3 example must ship with a lowering pass, so this returns
/// [`ParityPipelineError`] rather than `None`.
///
/// # Errors
///
/// Surfaces any parse, type-check, or lowering failure as
/// [`ParityPipelineError`].
pub fn compile(
    source: &str,
    file_id: u32,
) -> Result<(Ast, TypedAST, LoweredIr), ParityPipelineError> {
    let ast = flux_parser::parse(source, file_id, "example.flux")
        .map_err(|e| ParityPipelineError(format!("parse: {e}")))?;
    let typed = flux_types::type_check(&ast)
        .map_err(|e| ParityPipelineError(format!("type-check: {e}")))?;
    let lowered =
        flux_ir::lower(&ast, &typed).map_err(|e| ParityPipelineError(format!("lower: {e}")))?;
    Ok((ast, typed, lowered))
}

/// Runs the full parity check for one `.flux` source and returns the report.
///
/// The pipeline is: parse → type-check → lower → codegen(Swift, Kotlin) →
/// recognize each emitted source back into the structural [`ViewNode`] model.
///
/// # Errors
///
/// Returns [`ParityPipelineError`] if the source cannot be parsed, type-checked,
/// or lowered, or if either codegen backend emits source that the recognizer
/// cannot parse (the latter would itself indicate a codegen/parity drift). A
/// lowering failure is *not* swallowed — see the crate-level docs (issue 5).
pub fn check_parity(source: &str, file_id: u32) -> Result<ParityReport, ParityPipelineError> {
    let (ast, _typed, lowered) = compile(source, file_id)?;
    let dev = crate::model::from_ast(&ast);
    let swift_src = flux_codegen_swift::codegen(&lowered, &ast);
    let kotlin_src = flux_codegen_kotlin::codegen(&lowered, &ast);
    let swift = recognize_swift(&dev, &swift_src)
        .map_err(|e| ParityPipelineError(format!("swift recognize: {e}")))?;
    let kotlin = recognize_kotlin(&dev, &kotlin_src)
        .map_err(|e| ParityPipelineError(format!("kotlin recognize: {e}")))?;
    Ok(ParityReport { dev, swift, kotlin })
}
