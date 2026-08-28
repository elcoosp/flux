//! The structural model both paths are reduced to before comparison.
//!
//! Parity is proven by reducing the dev-path surface AST ([`flux_parser::Ast`]) and
//! each release-path emitted source (SwiftUI / Compose text) to the *same*
//! discriminated [`ViewNode`] tree, then asserting the three trees are
//! structurally identical. We compare structure — component/view graph, control
//! flow (`if`/`when`/`ForEach`/`match`) and value bindings (string literals) — not
//! source text, so cosmetic backend differences (Swift `VStack` vs Kotlin
//! `Column`, `\(` vs `${`) are correctly normalized away.
//!
//! The dev path drives the tree directly from the parsed AST: the AST is the
//! authoritative "what the user wrote" and is exactly what the release codegen
//! derives from, so reducing it to the structural [`ViewNode`] tree is the
//! faithful dev-side equivalent. State/handler/prop/lifecycle declarations are
//! skipped; only the view graph and control flow are retained.
//!
//! [`ViewNode`] is the single shared structural model, defined in
//! `flux-codegen-core` (the release codegen's source of truth) so the dev-path
//! AST reducer and the release-path lowered-IR walker ([`flux_codegen_core::view_tree`])
//! reduce to the *same* vocabulary and can be compared (and serialized to JSON)
//! deterministically (roadmap Phase 4).

/// The language-neutral structural view tree, shared with the release codegen.
pub use flux_codegen_core::ViewNode;

pub(crate) use crate::reduce::is_container;
pub use crate::reduce::{from_ast, normalize_view_name};
