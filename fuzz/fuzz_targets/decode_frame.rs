//! `cargo fuzz` target for the Flux wire decoder (LANE-D, task 1).
//!
//! Every input is fed through the three untrusted-frame decoders
//! (`Frame::from_init_bytes`, `Frame::from_delta_bytes`, `Frame::from_hello_bytes`).
//! The contract under test: an attacker-controlled buffer must NEVER cause a
//! panic. Every input either returns `Ok` or a typed `WireError`/`None` — the
//! decoder must not `unwrap()` on attacker bytes. libFuzzer aborts the process
//! on any panic (including the `panic!` inside `expect`/`unwrap`), which is
//! exactly the regression we are hardening against.
//
// NOTE: this crate is a standalone `cargo fuzz` workspace (it is NOT a member
// of the parent `flux` workspace, so `cargo fuzz` can manage its own toolchain
// and `libfuzzer-sys` dependency). Build/run it with:
//
//     cargo +nightly fuzz build -O decode_frame
//     cargo +nightly fuzz run -O decode_frame   # (ctrl-c after 60s to stop)

#![no_main]

use libfuzzer_sys::fuzz_target;
use flux_ir_serde::Frame;

fuzz_target!(|data: &[u8]| {
    // Each decoder must be total on arbitrary bytes: no panic, ever.
    let _init = Frame::from_init_bytes(data);
    let _delta = Frame::from_delta_bytes(data);
    let _hello = Frame::from_hello_bytes(data);
});
