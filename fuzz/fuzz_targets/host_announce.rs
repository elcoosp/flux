//! `cargo fuzz` target for the Flux host-announce frame decoder (T-604.19).
//!
//! Feeds every byte through `HostAnnounceFrame::from_bytes`. The contract:
//! attacker-controlled bytes must NEVER panic.
#![no_main]

use flux_ir_serde::HostAnnounceFrame;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = HostAnnounceFrame::from_bytes(data);
});
