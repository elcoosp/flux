//! `cargo fuzz` target for the Flux telemetry frame decoder (T-604.19).
//!
//! Feeds every byte through `TelemetryFrame::from_bytes`. The contract:
//! attacker-controlled bytes must NEVER panic.
#![no_main]

use flux_ir_serde::TelemetryFrame;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = TelemetryFrame::from_bytes(data);
});
