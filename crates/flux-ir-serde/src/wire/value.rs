//! `Value` wire codec (Appendix D §D.5).

use flux_syntax::Value;

use super::core::WireError;
use super::cursor::{Reader, Writer};

pub(crate) fn encode_value(w: &mut Writer, value: &Value) -> Result<(), WireError> {
    // Audit fix (H14): the list/record length prefixes are the encoder's
    // first touchpoint with user-authored sizes. A 70 000-item `List` prop
    // is legal Flux that previously panicked the pipeline thread inside
    // `u16_len`; now it surfaces as a typed
    // `WireError::LengthExceedsU16` at this boundary. Callers in the frame
    // composition path still `.expect(...)` as scaffolding until the
    // frame-level API is made fallible; recursive calls within this function
    // use `?` so a nested oversized collection propagates cleanly.
    w.u8(value.tag());
    match value {
        Value::Null => {}
        Value::Int(i) => w.u64(*i as u64),
        Value::Float(f) => {
            // Canonicalise NaN so any NaN bit pattern round-trips to one value
            // (Rust's `NaN != NaN`, so a non-canonical NaN would fail equality
            // on decode). Matches `Value::hash_into`'s treatment.
            let canonical = if f.is_nan() { f64::NAN } else { *f };
            w.u64(canonical.to_bits());
        }
        Value::Bool(b) => w.u8(u8::from(*b)),
        Value::Str(id) | Value::HandlerRef(id) => w.u32(*id),
        Value::List(items) => {
            w.count_prefix(items.len(), "value.list")?;
            for item in items {
                encode_value(w, item)?;
            }
        }
        Value::Record(fields) => {
            w.count_prefix(fields.len(), "value.record.fields")?;
            for (index, val) in fields {
                w.u16(*index);
                encode_value(w, val)?;
            }
        }
        // `Value` is `#[non_exhaustive]` (flux-syntax): all variants from the
        // type checker are handled above. An unknown variant means a new
        // `Value` variant was added without an encoder (audit P2.25). The tag
        // byte is already on the wire, so silencing this desyncs every
        // downstream frame.
        _ => unreachable!("unknown Value variant — add an encoder arm for it (audit P2.25)"),
    }
    Ok(())
}

/// Encodes a [`Value`] into a standalone Appendix D §D.5 blob (no frame header).
///
/// This is the on-the-wire storage encoding the host `StorageBackend`s persist:
/// a `set` writes this blob, a `get` decodes it back. The `flux-parity` harness
/// uses it to drive the persistence-parity trace (FLUX-082) without inventing a
/// second codec.
#[must_use]
pub fn encode_value_blob(value: &Value) -> Vec<u8> {
    let mut w = Writer::new();
    // Scaffold: `encode_value_blob` is used by the parity persistence tests
    // and a handful of fixtures — no caller has a way to surface a typed
    // encode error yet. Panic with the specific variant's message; once the
    // full to_bytes chain is fallible (audit H14 cascade), this becomes
    // `-> Result<Vec<u8>, WireError>`.
    encode_value(&mut w, value).expect("encode_value_blob: value exceeds wire length limits");
    w.into_vec()
}

/// Decodes a [`Value`] from a standalone Appendix D §D.5 blob.
///
/// Returns [`WireError`] on a truncated or corrupt blob — the exact failure a
/// host `StorageBackend.get` must catch and treat as `absent` (FLUX-080/081),
/// never propagate as a host crash.
pub fn decode_value_blob(blob: &[u8]) -> Result<Value, WireError> {
    let mut r = Reader::new(blob);
    decode_value(&mut r)
}

const TAG_NULL: u8 = 0x00;
const TAG_INT: u8 = 0x01;
const TAG_FLOAT: u8 = 0x02;
const TAG_BOOL: u8 = 0x03;
const TAG_STR: u8 = 0x04;
const TAG_HANDLER: u8 = 0x05;
const TAG_LIST: u8 = 0x06;
const TAG_RECORD: u8 = 0x07;

/// Maximum nesting depth accepted for a `Value` before decode fails closed.
///
/// A ~200 KB hostile frame can otherwise drive `decode_value` recursion to the
/// process stack limit (Rust stack overflow is not catchable) and abort the
/// whole dev server. Mirrors the parser's `MAX_PARSE_DEPTH` policy of bounding
/// input-driven recursion.
pub(crate) const MAX_VALUE_DEPTH: usize = 128;

pub(crate) fn decode_value(r: &mut Reader<'_>) -> Result<Value, WireError> {
    decode_value_at(r, 0)
}

fn decode_value_at(r: &mut Reader<'_>, depth: usize) -> Result<Value, WireError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(WireError::InvalidTag {
            tag: 0,
            context: "value.depth",
            at: r.pos(),
        });
    }
    let tag = r.u8("value.tag")?;
    match tag {
        TAG_NULL => Ok(Value::Null),
        TAG_INT => Ok(Value::Int(r.i64("value.int")?)),
        TAG_FLOAT => Ok(Value::Float(f64::from_bits(r.u64("value.float")?))),
        TAG_BOOL => Ok(Value::Bool(r.u8("value.bool")? != 0)),
        TAG_STR => Ok(Value::Str(r.u32("value.str")?)),
        TAG_HANDLER => Ok(Value::HandlerRef(r.u32("value.handler")?)),
        TAG_LIST => {
            let count = r.count("value.list.count")?;
            r.ensure_capacity(count as usize, "value.list")?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(decode_value_at(r, depth + 1)?);
            }
            Ok(Value::List(items))
        }
        TAG_RECORD => {
            let count = r.count("value.record.count")?;
            r.ensure_capacity(count as usize, "value.record")?;
            let mut fields = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let index = r.u16("value.record.index")?;
                let val = decode_value_at(r, depth + 1)?;
                fields.push((index, val));
            }
            Ok(Value::Record(fields))
        }
        other => Err(WireError::InvalidTag {
            tag: other,
            context: "value",
            at: r.pos() - 1,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit H14 regression: a user-authored `List` with more items than a
    /// `u16` prefix can hold must surface as `WireError::LengthExceedsU16`
    /// from `encode_value`, not a panic inside `Writer::u16_len`. This test
    /// pins the fallible-encoder contract: if a future change reverts
    /// `encode_value` to the panicking path, this test fails (or panics with
    /// a different message).
    #[test]
    fn encode_value_accepts_former_u16_limit_list() {
        // ADR-0059: v3 widens the list count to u32, so a 65 536-item list —
        // which used to be the size at which v2 failed — now encodes cleanly.
        // The u16 limit is no longer a boundary; a real overflow requires
        // >4 billion items, which cannot be built in a test.
        let former_u16_ceiling = (u16::MAX as usize) + 1;
        let items: Vec<Value> = (0..former_u16_ceiling).map(|_| Value::Null).collect();
        let value = Value::List(items);
        let mut w = Writer::new();
        encode_value(&mut w, &value)
            .expect("v3 list wider than u16::MAX must encode");
    }

    /// ADR-0059: v3 widens the record field count to u32, so a record wider
    /// than the v2 u16 limit now encodes. The old "rejects" test has been
    /// replaced by an "accepts" test; real overflow is unreachable in RAM.
    #[test]
    fn encode_value_accepts_former_u16_limit_record() {
        let former_u16_ceiling = (u16::MAX as usize) + 1;
        let fields: Vec<(flux_syntax::PropIdx, Value)> = (0..former_u16_ceiling)
            .map(|i| (flux_syntax::PropIdx::from(i as u16), Value::Null))
            .collect();
        let value = Value::Record(fields);
        let mut w = Writer::new();
        encode_value(&mut w, &value)
            .expect("v3 record wider than u16::MAX must encode");
    }

    /// A normal-sized list still round-trips through the fallible encoder.
    /// Guards against the migration accidentally breaking the happy path.
    #[test]
    fn encode_value_round_trips_small_list() {
        let value = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let mut w = Writer::new();
        encode_value(&mut w, &value).expect("small list encodes");
        // v3 (ADR-0059) writes the list count as u32, not u16:
        //   tag(1) + len(4) + 3 * (item-tag(1) + Int payload(8)) = 32 bytes.
        assert_eq!(w.buf_len(), 1 + 4 + 3 * 9);
    }
}

