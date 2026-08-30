//! Behavioural tests for the shared `flux-syntax` vocabulary.
//!
//! These exercise only the public API, per `AGENTS.md` §1.1.

use flux_syntax::{
    Child, ClosureRef, NodeKind, NodeRef, PropDiff, Props, Span, StringTable, TypeKind, Value,
};

#[test]
fn test_string_table_interns_equal_strings_to_same_id() {
    let mut table = StringTable::new();
    let first = table.intern("Column");
    let second = table.intern("Column");
    assert_eq!(first, second);
}

#[test]
fn test_string_table_interns_distinct_strings_to_distinct_ids() {
    let mut table = StringTable::new();
    assert_ne!(table.intern("Column"), table.intern("Row"));
}

#[test]
fn test_string_table_resolves_interned_id_back_to_text() {
    let mut table = StringTable::new();
    let id = table.intern("Button");
    assert_eq!(table.resolve(id), Some("Button"));
}

#[test]
fn test_string_table_resolve_of_unknown_id_is_none() {
    let table = StringTable::new();
    assert_eq!(table.resolve(7), None);
}

#[test]
fn test_string_table_interns_unicode_text() {
    let mut table = StringTable::new();
    let id = table.intern("counter — ✓ 数");
    assert_eq!(table.resolve(id), Some("counter — ✓ 数"));
}

#[test]
fn test_string_table_len_counts_unique_strings_only() {
    let mut table = StringTable::new();
    table.intern("a");
    table.intern("a");
    table.intern("b");
    assert_eq!(table.len(), 2);
}

#[test]
fn test_empty_string_table_is_empty() {
    assert!(StringTable::new().is_empty());
}

#[test]
fn test_span_contains_offset_inside_range() {
    let span = Span::new(0, 10, 20);
    assert!(span.contains(15));
}

#[test]
fn test_span_end_offset_is_exclusive() {
    let span = Span::new(0, 10, 20);
    assert!(!span.contains(20));
}

#[test]
fn test_span_len_is_byte_length() {
    assert_eq!(Span::new(0, 10, 20).len(), 10);
}

#[test]
fn test_span_join_covers_both_operands() {
    let joined = Span::new(3, 10, 12).join(Span::new(3, 40, 44));
    assert_eq!(joined, Span::new(3, 10, 44));
}

#[test]
fn test_props_get_returns_value_for_present_index() {
    let props = Props::from_fields(vec![(1, Value::Int(42))]);
    assert!(matches!(props.get(1), Some(Value::Int(42))));
}

#[test]
fn test_props_get_returns_none_for_absent_index() {
    let props = Props::from_fields(vec![(1, Value::Int(42))]);
    assert!(props.get(2).is_none());
}

#[test]
fn test_props_get_bool_falls_back_to_default_when_absent() {
    assert!(Props::default().get_bool(0, true));
}

#[test]
fn test_props_get_bool_reads_stored_bool() {
    let props = Props::from_fields(vec![(0, Value::Bool(false))]);
    assert!(!props.get_bool(0, true));
}

#[test]
fn test_props_get_handler_returns_none_for_non_handler_value() {
    let props = Props::from_fields(vec![(0, Value::Int(1))]);
    assert_eq!(props.get_handler(0), None);
}

#[test]
fn test_props_get_handler_returns_bound_handler_id() {
    let props = Props::from_fields(vec![(0, Value::HandlerRef(9))]);
    assert_eq!(props.get_handler(0), Some(9));
}

#[test]
fn test_props_get_str_resolves_through_string_table() {
    let mut table = StringTable::new();
    let id = table.intern("Increment");
    let props = Props::from_fields(vec![(0, Value::Str(id))]);
    assert_eq!(props.get_str(0, &table), Some("Increment"));
}

#[test]
fn test_props_hash_is_stable_for_equal_field_sets() {
    let left = Props::from_fields(vec![(0, Value::Int(1)), (1, Value::Bool(true))]);
    let right = Props::from_fields(vec![(0, Value::Int(1)), (1, Value::Bool(true))]);
    assert_eq!(left.hash(), right.hash());
}

#[test]
fn test_props_hash_differs_when_a_value_changes() {
    let left = Props::from_fields(vec![(0, Value::Int(1))]);
    let right = Props::from_fields(vec![(0, Value::Int(2))]);
    assert_ne!(left.hash(), right.hash());
}

#[test]
fn test_props_hash_is_order_independent() {
    let left = Props::from_fields(vec![(0, Value::Int(1)), (1, Value::Int(2))]);
    let right = Props::from_fields(vec![(1, Value::Int(2)), (0, Value::Int(1))]);
    assert_eq!(left.hash(), right.hash());
}

#[test]
fn test_props_hash_of_empty_field_set_is_deterministic() {
    assert_eq!(
        Props::default().hash(),
        Props::from_fields(Vec::new()).hash()
    );
}

#[test]
fn test_nan_float_props_hash_equal_to_themselves() {
    let left = Props::from_fields(vec![(0, Value::Float(f64::NAN))]);
    let right = Props::from_fields(vec![(0, Value::Float(f64::NAN))]);
    assert_eq!(left.hash(), right.hash());
}

#[test]
fn test_node_kind_round_trips_through_its_wire_tag() {
    for kind in NodeKind::ALL {
        assert_eq!(NodeKind::from_tag(kind.tag()), Some(kind));
    }
}

#[test]
fn test_node_kind_rejects_unknown_wire_tag() {
    assert_eq!(NodeKind::from_tag(200), None);
}

#[test]
fn test_node_kind_component_tag_matches_appendix_c() {
    assert_eq!(NodeKind::Component.tag(), 0);
}

#[test]
fn test_node_kind_screen_tag_matches_appendix_c() {
    assert_eq!(NodeKind::Screen.tag(), 6);
}

#[test]
fn test_value_tag_of_null_matches_appendix_c_encoding() {
    assert_eq!(Value::Null.tag(), 0x00);
}

#[test]
fn test_value_tag_of_record_matches_appendix_c_encoding() {
    assert_eq!(Value::Record(Vec::new()).tag(), 0x07);
}

#[test]
fn test_type_kind_list_of_int_reports_int_element() {
    let list = TypeKind::List(Box::new(TypeKind::Int));
    assert_eq!(list.element_type(), Some(&TypeKind::Int));
}

#[test]
fn test_type_kind_primitives_have_no_element_type() {
    assert_eq!(TypeKind::Bool.element_type(), None);
}

#[test]
fn test_type_kind_var_is_not_concrete() {
    assert!(!TypeKind::Var(0).is_concrete());
}

#[test]
fn test_type_kind_nested_var_makes_outer_type_non_concrete() {
    assert!(!TypeKind::List(Box::new(TypeKind::Var(1))).is_concrete());
}

#[test]
fn test_type_kind_string_is_concrete() {
    assert!(TypeKind::String.is_concrete());
}

#[test]
fn test_node_ref_child_node_ids_skips_splice_boundaries() {
    let node = NodeRef {
        id: 1,
        kind: NodeKind::Component,
        component_id: 0,
        props: Props::default(),
        children: vec![
            Child::Node(2),
            Child::Splice {
                items: vec![(10, 3), (11, 4)],
            },
        ],
        handlers: Vec::new(),
        span: Span::new(0, 0, 1),
    };
    assert_eq!(node.child_node_ids().collect::<Vec<_>>(), vec![2, 3, 4]);
}

#[test]
fn test_prop_diff_is_empty_when_no_changes_or_removals() {
    assert!(PropDiff::default().is_empty());
}

#[test]
fn test_prop_diff_with_a_removal_is_not_empty() {
    let diff = PropDiff {
        changes: Vec::new(),
        removals: vec![3],
    };
    assert!(!diff.is_empty());
}

#[test]
fn test_closure_ref_bytecode_range_matches_offset_and_length() {
    let closure = ClosureRef {
        hash: 0,
        bytecode_offset: 16,
        bytecode_len: 4,
        captured_signals: Vec::new(),
        span: Span::new(0, 0, 0),
        excerpt: None,
    };
    assert_eq!(closure.bytecode_range(), 16..20);
}
