//! T-304 regression: oracle list-op operand positions match the compiler.

use flux_syntax::Value;
use flux_vm_ref::{InMemorySignals, run};

#[test]
fn list_insert_does_not_panic() {
    // LOAD_INT_CONST r1, 42; ALLOC_LIST r0; LIST_INSERT r0, idx=0, val=r1; HALT
    let prog = [
        0xB0, 1, 42, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r1, 42
        0x80, 0, 0, 0, // ALLOC_LIST r0, cap=0
        0x85, 0, 0, 1, // LIST_INSERT r0, idx=0, val=r1
        0x00, // HALT
    ];
    let result = run(&prog, &mut InMemorySignals::default(), Value::Null);
    assert!(
        result.is_ok(),
        "list insert should not panic with 3-byte operands (T-304)"
    );
}

#[test]
fn list_remove_does_not_panic() {
    // ALLOC_LIST r0; LOAD_INT_CONST r1, 42; LIST_INSERT r0, 0, r1; LIST_REMOVE r0, 0; HALT
    let prog = [
        0x80, 0, 0, 0, // ALLOC_LIST r0, cap=0
        0xB0, 1, 42, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r1, 42
        0x85, 0, 0, 1, // LIST_INSERT r0, idx=0, val=r1
        0x86, 0, 0, // LIST_REMOVE r0, idx=0
        0x00, // HALT
    ];
    let result = run(&prog, &mut InMemorySignals::default(), Value::Null);
    assert!(
        result.is_ok(),
        "list remove should not panic with 2-byte operands (T-304)"
    );
}
