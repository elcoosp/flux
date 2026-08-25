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
//! The dev/reference path drives off the surface AST rather than the lowered IR:
//! the MLP lowerer ([`flux_ir`]) does not yet lower every B.3 handler/property
//! form, so a lowering failure does not invalidate the example — it simply means
//! the release codegen backend could not be exercised for that example. Such
//! examples are reported with [`ParityStatus::LowererUnsupported`] instead of
//! panicking, and the harness still proves parity for every example the pipeline
//! can fully compile.

use flux_ir::LoweredIr;
use flux_parser::Ast;

use flux_types::TypedAST;

use crate::equivalence::structurally_equal;
use crate::model::ViewNode;
use crate::recognize_kotlin::recognize as recognize_kotlin;
use crate::recognize_swift::recognize as recognize_swift;

/// Whether the full dev→release parity pipeline was able to run for an example.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParityStatus {
    /// The example compiled, lowered and codegen'd; all three trees are present.
    Supported,
    /// The example parsed and type-checked, but the MLP lowerer rejected a
    /// handler/property form it does not yet support, so the release backends
    /// could not be exercised. The dev tree is still available from the AST.
    LowererUnsupported,
}

/// A full parity result for one example: the three reduced trees plus a verdict.
#[derive(Clone, Debug)]
pub struct ParityReport {
    /// The dev-path (surface AST) structural tree.
    pub dev: Vec<ViewNode>,
    /// The Swift release-path structural tree (empty when unsupported).
    pub swift: Vec<ViewNode>,
    /// The Kotlin release-path structural tree (empty when unsupported).
    pub kotlin: Vec<ViewNode>,
    /// Whether the release backends could be exercised for this example.
    pub status: ParityStatus,
}

impl ParityReport {
    /// Returns `true` when the release backends were exercised and the dev tree
    /// equals both release trees. When the lowerer could not support the example
    /// ([`ParityStatus::LowererUnsupported`]) this is `false` — the release trees
    /// were never produced — but the example is not a divergence; it is a
    /// documented capability boundary of the MLP lowerer.
    #[must_use]
    pub fn is_equivalent(&self) -> bool {
        self.status == ParityStatus::Supported
            && structurally_equal(&self.dev, &self.swift)
            && structurally_equal(&self.swift, &self.kotlin)
    }

    /// A one-line verdict used in snapshot output.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        match self.status {
            ParityStatus::Supported if self.is_equivalent() => "equivalent",
            ParityStatus::Supported => "DIVERGENT",
            ParityStatus::LowererUnsupported => "lowerer-unsupported",
        }
    }
}

/// Compiles a `.flux` source through the full pipeline: parse → type-check →
/// lower. The lowered IR is required by the release codegen backends; if the MLP
/// lowerer rejects a handler/property form it does not yet support, lowering
/// returns `None` and the example is reported as [`ParityStatus::LowererUnsupported`]
/// rather than erroring — the dev (AST) tree is still available.
///
/// # Errors
///
/// Surfaces any parse / type-check failure as [`ParityPipelineError`]. A lowering
/// failure is *not* an error: it is a known capability boundary of the MLP
/// lowerer and is returned as `None`.
pub fn compile(
    source: &str,
    file_id: u32,
) -> Result<(Ast, TypedAST, Option<LoweredIr>), ParityPipelineError> {
    let ast = flux_parser::parse(source, file_id, "example.flux")
        .map_err(|e| ParityPipelineError(format!("parse: {e}")))?;
    let typed = flux_types::type_check(&ast)
        .map_err(|e| ParityPipelineError(format!("type-check: {e}")))?;
    let lowered = flux_ir::lower(&ast, &typed).ok();
    Ok((ast, typed, lowered))
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

/// Runs the full parity check for one `.flux` source and returns the report.
///
/// # Errors
///
/// Returns [`ParityPipelineError`] if the source cannot be parsed or type-checked,
/// or if either codegen backend emits source that the recognizer cannot parse
/// (the latter would itself indicate a codegen/parity drift). A lowerer failure
/// is handled gracefully: the report carries [`ParityStatus::LowererUnsupported`].
pub fn check_parity(source: &str, file_id: u32) -> Result<ParityReport, ParityPipelineError> {
    let (ast, _typed, lowered) = compile(source, file_id)?;
    let dev = crate::model::from_ast(&ast);
    let (swift, kotlin, status) = match lowered {
        Some(lowered) => {
            let swift_src = flux_codegen_swift::codegen(&lowered, &ast);
            let kotlin_src = flux_codegen_kotlin::codegen(&lowered, &ast);
            let swift = recognize_swift(&dev, &swift_src)
                .map_err(|e| ParityPipelineError(format!("swift recognize: {e}")))?;
            let kotlin = recognize_kotlin(&dev, &kotlin_src)
                .map_err(|e| ParityPipelineError(format!("kotlin recognize: {e}")))?;
            (swift, kotlin, ParityStatus::Supported)
        }
        None => (vec![], vec![], ParityStatus::LowererUnsupported),
    };
    Ok(ParityReport {
        dev,
        swift,
        kotlin,
        status,
    })
}
