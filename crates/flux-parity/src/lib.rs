//! Parity harness: dev-mode VM execution versus release-mode generated code.
//!
//! The core guarantee of Flux is that what you see in dev mode is what ships
//! (BR-004). Dev mode interprets the lowered reactive IR through `flux-vm-ref`;
//! release mode runs generated Swift/Kotlin. This crate proves the two paths are
//! behaviorally equivalent for every Appendix B.3 example by reducing the
//! **dev-path reactive tree** ([`flux_ir::LoweredIr`]) and the **release-path
//! emitted source** (SwiftUI / Compose) to one shared, language-neutral
//! [`ViewNode`] structural model and asserting the three trees are identical.
//!
//! The ForEach empty-splice is intentional (FLUX-014): keyed items are reconciled
//! at runtime by the host, so the lowered IR carries an empty `ForEach` splice and
//! both codegen backends render an empty `ForEach`/`items` wrapper. Parity asserts
//! this empty body matches in all three paths — it is the expected, faithful
//! shape, never a divergence.
//!
//! Implemented by FLUX-023.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod bridge;
mod cache;
mod equivalence;
mod error_frame;
mod harness;
mod model;
mod persistence;
mod recognize_kotlin;
mod recognize_swift;
mod reduce;
mod relation;
mod sources;
mod tokenize;
pub mod trace;

pub use cache::{
    CacheOp, ImageCacheBackend, LruImageCache, assert_cache_parity, default_cache_script,
    run_cache_script,
};
pub use error_frame::{
    CorpusFrame, HostDecoder, ReferenceDecoder, Rejection, WireErrorKind,
    assert_error_frame_parity, default_corpus,
};
pub use harness::{
    ComponentUnderTest, InteractionOutcome, RenderError, render_component, run_tap,
    signal_after_tap,
};
pub use model::{ViewNode, from_ast, normalize_view_name};
pub use persistence::{
    GetResult, InMemoryStorageBackend, StorageBackend, StoreOp, StoreValue, assert_storage_parity,
    default_persistence_script, run_storage_script,
};
pub use recognize_kotlin::{KotlinRecognitionError, recognize as recognize_kotlin};
pub use recognize_swift::{SwiftRecognitionError, recognize as recognize_swift};
pub use relation::{ParityPipelineError, ParityReport, check_parity, compile};
pub use sources::all_examples;
