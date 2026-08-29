//! Render-perf harness for the Flux iOS/Android convergence question (PRD-J, ADR-0048).
//!
//! This crate is the **platform-neutral core** of the harness: the shared metric
//! schema ([`metric`]), the deterministic driver ([`driver`]), and the CI gate
//! predicate ([`gate`]). The host adapters that actually time the iOS
//! `FluxUIKit` reconciler and the Android `ShadowTreeRenderer` live in
//! `runtimes/` (they need a device/simulator), but they feed this same schema so
//! DevTools (PRD-P) can later consume the records too.
//!
//! No new opcodes or wire fields are introduced — this is measurement + a
//! decision, not a protocol change.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

pub mod driver;
pub mod gate;
pub mod metric;

pub use driver::{FixtureTree, HarnessDriver, MeasureFn};
pub use gate::{Budgets, GateVerdict, evaluate};
pub use metric::{LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario};
