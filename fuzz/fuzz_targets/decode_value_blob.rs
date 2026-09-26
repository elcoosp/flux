//! `cargo fuzz` target for the Flux value-blob decoder (T-604.19).
//!
//! Feeds every byte through `decode_value_blob`. The contract: attacker-
//! controlled bytes must NEVER panic; every input returns `Ok` or an `Err`,
//! never an `unwrap`/`expect` on attacker bytes.
#![no_main]

use flux_ir_serde::decode_value_blob;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = decode_value_blob(data);
});
