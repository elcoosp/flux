//! T-304 regression: oracle list-op operand positions match the compiler.

use flux_syntax::{StringTable, Value};
use flux_vm_ref::{InMemorySignals, run};

#[test]
fn list_insert_does_not_panic() {
    // F1: LIST_INSERT takes its index from a *register*, not a literal byte.
    // LOAD_INT_CONST r1, 42; LOAD_INT_CONST r2, 0; ALLOC_LIST r0;
    // LIST_INSERT r0, idx=r2, val=r1; HALT
    let prog = [
        0xB0, 1, 42, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r1, 42
        0xB0, 2, 0, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r2, 0 (index)
        0x80, 0, 0, 0, // ALLOC_LIST r0, cap=0
        0x85, 0, 2, 1,    // LIST_INSERT r0, idx=r2, val=r1
        0x00, // HALT
    ];
    let result = run(
        &prog,
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Null,
    );
    assert!(
        result.is_ok(),
        "list insert should not panic with 3-byte operands (T-304)"
    );
}

#[test]
fn list_remove_does_not_panic() {
    // F1: LIST_REMOVE takes its index from a register, not a literal byte.
    // ALLOC_LIST r0; LOAD_INT_CONST r1, 42; LOAD_INT_CONST r2, 0;
    // LIST_INSERT r0, r2, r1; LIST_REMOVE r0, r2; HALT
    let prog = [
        0x80, 0, 0, 0, // ALLOC_LIST r0, cap=0
        0xB0, 1, 42, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r1, 42
        0xB0, 2, 0, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r2, 0 (index)
        0x85, 0, 2, 1, // LIST_INSERT r0, idx=r2, val=r1
        0x86, 0, 2,    // LIST_REMOVE r0, idx=r2
        0x00, // HALT
    ];
    let result = run(
        &prog,
        &mut InMemorySignals::default(),
        &StringTable::new(),
        Value::Null,
    );
    assert!(
        result.is_ok(),
        "list remove should not panic with 2-byte operands (T-304)"
    );
}
