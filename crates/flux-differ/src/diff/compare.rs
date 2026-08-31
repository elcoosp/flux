use ahash::AHashSet;
use flux_ir::{IRArena, NodeView};
use flux_syntax::{HandlerId, PropDiff, PropIdx, StringTable, Value};

pub(crate) fn values_equal(a: &Value, b: &Value, ta: &StringTable, tb: &StringTable) -> bool {
    match (a, b) {
        (Value::Str(id_a), Value::Str(id_b)) => match (ta.resolve(*id_a), tb.resolve(*id_b)) {
            (Some(sa), Some(sb)) => sa == sb,
            _ => id_a == id_b,
        },
        (Value::List(la), Value::List(lb)) => {
            la.len() == lb.len() && la.iter().zip(lb).all(|(x, y)| values_equal(x, y, ta, tb))
        }
        (Value::Record(ra), Value::Record(rb)) => {
            ra.len() == rb.len()
                && ra
                    .iter()
                    .zip(rb)
                    .all(|((ka, x), (kb, y))| ka == kb && values_equal(x, y, ta, tb))
        }
        _ => a == b,
    }
}

/// `true` when every prop key maps to the same value in both nodes.
///
/// Prefers the arena-stored prop hash (an O(1) `u64` compare) over unpacking
/// both cold blobs — see `IRArena::props_hash`. The hash is computed from all
/// `(PropIdx, Value)` fields at pack time, so a mismatch implies the fields
/// differ. When the hashes match we re-check the actual fields as a guard, and
/// compare interned strings by *content* (see [`values_equal`]) so literal-text
/// edits are not masked by positional `StringId` interning.
pub(crate) fn props_equal(
    o: &NodeView<'_>,
    n: &NodeView<'_>,
    old: &IRArena,
    new: &IRArena,
) -> bool {
    if o.props_hash() != n.props_hash() {
        return false;
    }
    let of = o.props();
    let of = of.fields();
    let nf = n.props();
    let nf = nf.fields();
    of.len() == nf.len()
        && of.iter().zip(nf).all(|((ka, va), (kb, vb))| {
            ka == kb && values_equal(va, vb, old.string_table(), new.string_table())
        })
}

/// Computes the [`PropDiff`] between two nodes.
///
/// The change list is built with content-aware value comparison (see
/// [`values_equal`]) so a literal-text edit — where the positional `StringId`
/// is unchanged but the resolved text differs — appears as a real change and
/// is shipped to the host on the `Patch::Update`.
pub(crate) fn props_diff(
    o: &NodeView<'_>,
    n: &NodeView<'_>,
    old: &IRArena,
    new: &IRArena,
) -> PropDiff {
    let o_fields = o.props();
    let o_fields = o_fields.fields();
    let n_fields = n.props();
    let n_fields = n_fields.fields();
    let changes: Vec<(PropIdx, Value)> = n_fields
        .iter()
        .filter(|(k, v)| {
            !o_fields.iter().any(|(ok, ov)| {
                ok == k && values_equal(ov, v, old.string_table(), new.string_table())
            })
        })
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    let removals: Vec<PropIdx> = o_fields
        .iter()
        .filter(|(k, _)| !n_fields.iter().any(|(nk, _)| nk == k))
        .map(|(k, _)| *k)
        .collect();
    PropDiff { changes, removals }
}

/// `true` when both nodes bind the same handler ids AND every shared handler's
/// closure body is byte-identical.
///
/// Comparing content — not just ids — is required for hot reload: a prop
/// thunk (e.g. an interpolated string literal) keeps its stable `HandlerId`
/// across edits while its bytecode changes. An id-only compare would report
/// "no change" and suppress the `Patch::Handler` that drives the host's
/// re-materialize, silently breaking hot reload (FLUX-019 regression).
pub(crate) fn handlers_equal(
    old: &IRArena,
    new: &IRArena,
    o_handlers: &[HandlerId],
    n_handlers: &[HandlerId],
) -> bool {
    let o_set: AHashSet<HandlerId> = o_handlers.iter().copied().collect();
    let n_set: AHashSet<HandlerId> = n_handlers.iter().copied().collect();
    if o_set != n_set {
        return false;
    }
    o_set
        .iter()
        .all(|hid| match (old.closure(*hid), new.closure(*hid)) {
            (Some(a), Some(b)) => a.bytecode == b.bytecode,
            _ => false,
        })
}
