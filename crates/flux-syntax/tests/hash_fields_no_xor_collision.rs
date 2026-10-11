//! FLUX-F56: `Props::from_fields` uses an XOR-fold to compute its
//! content hash, which cancels identical duplicate fields. Two identical
//! fields hash equal to the empty props. This test is `#[ignore]`d and
//! pins the current collision. When the fix lands, un-ignore and flip.

use flux_syntax::{PropIdx, Props, Value};

#[test]
#[ignore = "FLUX-F56: XOR-fold cancels duplicate PropIdx entries"]
fn duplicate_props_do_not_hash_equal_to_empty() {
    let empty = Props::from_fields(vec![]);
    let dup = Props::from_fields(vec![
        (PropIdx::from(5u16), Value::Int(7)),
        (PropIdx::from(5u16), Value::Int(7)),
    ]);
    assert_ne!(
        dup.hash(),
        empty.hash(),
        "duplicate props must not hash the same as no props"
    );
}

/// Positive control: two *different* fields hash differently from empty.
#[test]
fn distinct_props_hash_differently_from_empty() {
    let empty = Props::from_fields(vec![]);
    let single = Props::from_fields(vec![(PropIdx::from(5u16), Value::Int(7))]);
    assert_ne!(single.hash(), empty.hash());
}

/// Positive control: order independence still holds for distinct fields.
#[test]
fn distinct_fields_are_order_independent() {
    let a = Props::from_fields(vec![
        (PropIdx::from(1u16), Value::Int(1)),
        (PropIdx::from(2u16), Value::Int(2)),
    ]);
    let b = Props::from_fields(vec![
        (PropIdx::from(2u16), Value::Int(2)),
        (PropIdx::from(1u16), Value::Int(1)),
    ]);
    assert_eq!(a.hash(), b.hash(), "distinct fields hash order-independently");
}
