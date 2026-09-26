//! `cargo fuzz` target for the Flux dispatch-report decoder (T-604.19).
//!
//! Feeds every byte through `DispatchReport::from_bytes`. The contract:
//! attacker-controlled bytes must NEVER panic; every input returns `Some` or
//! `None`, never unwraps on attacker bytes.
#![no_main]

use flux_devserver::DispatchReport;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = DispatchReport::from_bytes(data);
});
