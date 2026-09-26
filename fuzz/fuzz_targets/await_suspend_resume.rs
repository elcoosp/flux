//! `cargo fuzz` target for the Flux AwaitSuspend / Resume decoder (T-604.19).
//!
//! Feeds every byte through `AwaitSuspendFrame::from_bytes` and
//! `ResumeFrame::from_bytes`. The contract: attacker-controlled bytes must
//! NEVER panic.
#![no_main]

use flux_ir_serde::{AwaitSuspendFrame, ResumeFrame};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = AwaitSuspendFrame::from_bytes(data);
    let _ = ResumeFrame::from_bytes(data);
});
