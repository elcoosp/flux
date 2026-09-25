//! `cargo fuzz` target for the Flux parser (T-604.2).
//!
//! Every input is fed through the parser. The contract under test:
//! attacker-controlled bytes must NEVER cause a panic. Every input either
//! parses to an [`Ast`] or returns a typed [`ParseError`] — the parser must
//! not `unwrap()` on attacker bytes, and the recursion-depth guard must
//! trigger before the call stack overflows.
//
// NOTE: this crate is a standalone `cargo fuzz` workspace (it is NOT a member
// of the parent `flux` workspace, so `cargo fuzz` can manage its own toolchain
// and `libfuzzer-sys` dependency). Build/run it with:
//
//     cargo +nightly fuzz build -O parse_flux
//     cargo +nightly fuzz run -O parse_flux   # (ctrl-c after 60s to stop)

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Lossy conversion: invalid UTF-8 becomes replacement chars rather
    // than crashing — the parser handles any `&str`.
    let source = String::from_utf8_lossy(data);
    let _ = flux_parser::parse(&source, 0, "<fuzz>");
});
