//! Parity harness: dev-mode VM execution versus release-mode generated code.
//!
//! The core guarantee of Flux is that what you see in dev mode is what ships
//! (BR-004). Dev mode interprets bytecode; release mode runs generated
//! Swift/Kotlin. This crate runs the same scenarios through both paths and
//! asserts the resulting state is identical.
//!
//! The dev side executes via `flux-vm-ref`, the reference VM. The release side
//! compiles and runs generated sources in minimal harness apps.
//!
//! Implemented by FLUX-023.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
