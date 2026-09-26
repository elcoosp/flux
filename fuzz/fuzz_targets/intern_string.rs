//! `cargo fuzz` target for the Flux intern-string frame decoder (T-604.19).
//!
//! Feeds every byte through `Frame::from_intern_string_bytes`. The contract:
//! attacker-controlled bytes must NEVER panic; every input returns `Some` or
//! `None`, never unwraps on attacker bytes.
#![no_main]

use flux_ir_serde::Frame;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = Frame::from_intern_string_bytes(data);
});
