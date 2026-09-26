//! `cargo fuzz` target for the Flux debug-command frame decoder (T-604.19).
//!
//! Feeds every byte through `DebugCommandFrame::from_bytes`. The contract:
//! attacker-controlled bytes must NEVER panic.
#![no_main]

use flux_ir_serde::DebugCommandFrame;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = DebugCommandFrame::from_bytes(data);
});
