//! `cargo fuzz` target for the Flux bytecode validator (T-604.19).
//!
//! Feeds every byte through `validate_bytecode`. The contract: attacker-
//! controlled bytes must NEVER panic; every input returns `Ok(())` or an
//! `Err`, never an `unwrap`/`expect` on attacker bytes.
#![no_main]

use flux_ir_serde::validate_bytecode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = validate_bytecode(data);
});
