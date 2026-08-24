//! Reference implementation of the Flux virtual machine (Appendix E).
//!
//! This VM is a **test oracle only**. It never ships in a host app and never
//! executes user code in production: the production VMs are native Swift and
//! Kotlin (ADR-0002). Its purpose is to give the Rust side of the toolchain an
//! executable definition of the ISA, so that lowering tests
//! (`flux-ir`) and the parity harness (`flux-parity`) can assert what emitted
//! bytecode actually does.
//!
//! Behavioural agreement between all three implementations is pinned by the
//! golden ISA vectors in `/tests/isa-vectors/`, which are the source of truth
//! for VM semantics — not this crate.
//!
//! Implemented by FLUX-005.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
