//! Headless `.flux` app-test harness integration tests (FLUX-034).
//!
//! Proves the user-facing API in `flux_parity::harness`:
//!  1. a `.flux` component lowers headlessly and its structural shadow tree is
//!     assertable;
//!  2. a synthetic tap expressed as an Appendix-E ISA program updates a signal
//!     cell that the test can read back.

use flux_parity::{render_component, run_tap, signal_after_tap};
use flux_syntax::{SignalId, Value};

/// A minimal counter component: a `Column` containing a `Text` and a `Button`.
/// Syntax follows the indentation-delimited "dream" surface (ADR-0029).
const COUNTER: &str = r#"
compo Counter
    $count: Int = 0

    Column gap: 8.0
        Text text: "tapped {count} times"
        Button text: "Increment", onPress: || { count = count + 1 }
"#;

#[test]
fn headless_render_builds_view_tree() {
    let comp = render_component(COUNTER, 1).expect("counter lowers headlessly");
    // The top-level component is present and named.
    assert_eq!(comp.count("Counter"), 1);
    // Its structural children (Column > Text, Button) are recoverable.
    assert_eq!(comp.count("Column"), 1);
    assert_eq!(comp.count("Text"), 1);
    assert_eq!(comp.count("Button"), 1);
    // The first Button node exists for deeper assertions.
    assert!(comp.find_first("Button").is_some());
}

#[test]
fn headless_render_rejects_bad_source() {
    let err = render_component("component { not valid flux", 2).unwrap_err();
    assert!(err.to_string().contains("headless render error"));
}

/// A handler that writes `Int(7)` into signal cell 1, then halts.
///   LOAD_INT_CONST r0, 7   (0xB0, dst=0, i64=7)
///   WRITE_SIGNAL   id=1, r0 (0x11, u32 id=1, u8 src=0)
///   HALT                  (0x00)
fn tap_writes_signal_one() -> Vec<u8> {
    let mut p = Vec::new();
    p.push(0xB0); // LOAD_INT_CONST
    p.push(0); // dst = r0
    p.extend_from_slice(&7_i64.to_le_bytes()); // value = 7
    p.push(0x11); // WRITE_SIGNAL
    p.extend_from_slice(&1_u32.to_le_bytes()); // signal id = 1
    p.push(0); // src = r0
    p.push(0x00); // HALT
    p
}

#[test]
fn synthetic_tap_updates_signal() {
    let program = tap_writes_signal_one();
    let out = run_tap(&program, Value::Null).expect("tap runs");
    // Signal cell 1 was written with Int(7).
    let written: Vec<_> = out
        .signals
        .iter()
        .filter(|(id, _)| *id == SignalId::from(1u32))
        .collect();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].1, Value::Int(7));

    // And the convenience helper returns exactly that value.
    let v = signal_after_tap(&program, Value::Null, SignalId::from(1u32))
        .expect("tap runs")
        .expect("signal 1 was written");
    assert_eq!(v, Value::Int(7));
}

#[test]
fn synthetic_tap_reports_vm_fault() {
    // A single invalid opcode must surface as a RenderError, not a panic.
    let err = signal_after_tap(&[0xFF], Value::Null, SignalId::from(1u32)).unwrap_err();
    assert!(err.to_string().contains("vm:"));
}
