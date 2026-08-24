//! Appendix D patch encoding (FLUX-013).
//!
//! [`serialize_patches`] turns a slice of [`Patch`] into the Appendix D §D.2
//! binary layout, interning any newly-seen strings into the supplied
//! [`StringTable`] delta. [`deserialize_patches`] is the round-trip decoder
//! used by the test suite only; production decoders live in Swift/Kotlin.
//!
//! Content addressing is provided by [`hash_props`] and [`hash_closure`], both
//! BLAKE3 digests used by the host-side cache (Appendix D §D.14).

use blake3::Hasher;
use flux_ir::ClosureIR;
use flux_syntax::{Patch, PropIdx, SignalId, StringTable, Value};

use crate::frame::Frame;
use crate::wire::WireError;

/// Serializes a patch stream into a `Delta` wire frame (Appendix D §D.1 + §D.2).
///
/// The encoder does **not** mutate `table`; the caller passes the table that
/// owns every `StringId` referenced by the patches, and the host app is
/// expected to already hold the same table from the preceding `Init` frame.
/// Strings that the host cannot resolve are a protocol error upstream, so
/// interning here would only mask a desync.
///
/// # Examples
///
/// ```
/// use flux_ir::ClosureIR;
/// use flux_ir_serde::{deserialize_patches, serialize_patches};
/// use flux_syntax::{Patch, SignalId, Span, StringTable};
///
/// let table = StringTable::new();
/// let patches = vec![Patch::Remove { id: 42 }];
/// let bytes = serialize_patches(&patches, &table, &[]);
/// let (back, closures) = deserialize_patches(&bytes).unwrap();
/// assert_eq!(serialize_patches(&back, &table, &closures), bytes);
/// // The example ships no handlers, so the closure stream is empty.
/// let _ = ClosureIR::new(
///     flux_syntax::HandlerId::from(1u32),
///     vec![],
///     vec![],
///     Span::new(0, 0, 0),
/// );
/// ```
#[must_use]
pub fn serialize_patches(
    patches: &[Patch],
    _table: &StringTable,
    closures: &[ClosureIR],
) -> Vec<u8> {
    Frame::delta(0, 0, patches, &[], closures).to_bytes()
}

/// Round-trip decoder for tests only (production decoders are Swift/Kotlin).
///
/// Returns [`WireError`] on any truncated buffer or unknown tag, which the
/// test harness treats as a serialization bug rather than a recoverable frame.
/// The returned `Vec<ClosureIR>` is the frame's handler section (Gap G1); it is
/// empty when the frame carried no handlers.
///
/// # Examples
///
/// ```
/// use flux_ir_serde::{deserialize_patches, serialize_patches};
/// use flux_syntax::{Patch, StringTable};
///
/// let patches = vec![Patch::Remove { id: 42 }];
/// let bytes = serialize_patches(&patches, &StringTable::new(), &[]);
/// let (back, _closures) = deserialize_patches(&bytes).unwrap();
/// // `Patch` does not derive `Eq`, so compare the canonical encodings.
/// assert_eq!(serialize_patches(&back, &StringTable::new(), &[]), bytes);
/// ```
pub fn deserialize_patches(bytes: &[u8]) -> Result<(Vec<Patch>, Vec<ClosureIR>), WireError> {
    let frame = Frame::from_delta_bytes(bytes)?;
    Ok((frame.patches, frame.closures))
}

/// BLAKE3 content hash of a props map.
///
/// The digest is computed order-independently: each `(index, value)` pair is
/// hashed with `index` in little-endian and XOR-folded into an 8-byte digest,
/// so two prop maps that differ only in field order hash identically (Appendix
/// D §D.14). Floats are canonicalised through [`Value::hash_into`].
///
/// # Examples
///
/// ```
/// use flux_ir_serde::hash_props;
/// use flux_syntax::Value;
///
/// let a = vec![(0u16, Value::Int(1)), (1u16, Value::Bool(true))];
/// let b = vec![(1u16, Value::Bool(true)), (0u16, Value::Int(1))];
/// assert_eq!(hash_props(&a), hash_props(&b), "order must not matter");
/// ```
#[must_use]
pub fn hash_props(fields: &[(PropIdx, Value)]) -> u64 {
    let mut accumulator: u64 = 0xcbf2_9ce4_8422_2325;
    for (index, value) in fields {
        let mut hasher = Hasher::new();
        hasher.update(&index.to_le_bytes());
        value.hash_into(&mut hasher);
        let mut digest = [0_u8; 8];
        digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        accumulator ^= u64::from_le_bytes(digest);
    }
    accumulator
}

/// BLAKE3 content hash of a closure body.
///
/// The digest covers the bytecode bytes and the captured signal IDs (Appendix
/// D §D.7). Two closures with identical bytecode but different captures hash
/// differently, because captures change behaviour against the signal graph.
///
/// # Examples
///
/// ```
/// use flux_ir_serde::hash_closure;
/// use flux_syntax::SignalId;
///
/// let code = vec![0x00u8, 0x10, 0x20];
/// let a = hash_closure(&code, &[SignalId::from(1u32)]);
/// let b = hash_closure(&code, &[SignalId::from(1u32)]);
/// assert_eq!(a, b);
/// ```
#[must_use]
pub fn hash_closure(bytecode: &[u8], signals: &[SignalId]) -> u64 {
    let mut hasher = Hasher::new();
    hasher.update(&(bytecode.len() as u32).to_le_bytes());
    hasher.update(bytecode);
    hasher.update(&(signals.len() as u32).to_le_bytes());
    for signal in signals {
        hasher.update(&signal.to_le_bytes());
    }
    let mut digest = [0_u8; 8];
    digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
    u64::from_le_bytes(digest)
}
