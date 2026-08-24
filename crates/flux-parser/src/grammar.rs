//! The pest-generated grammar entry point.

use pest_derive::Parser;

/// The generated Flux grammar parser.
///
/// The grammar itself lives in `src/flux.pest` and is normative against
/// Appendix B of `/docs/spec/mlp-appendices.md`.
#[derive(Debug, Parser)]
#[grammar = "flux.pest"]
pub(crate) struct FluxGrammar;
