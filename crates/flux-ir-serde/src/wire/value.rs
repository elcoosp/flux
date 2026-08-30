//! `Value` wire codec (Appendix D §D.5).

use flux_syntax::Value;

use super::core::WireError;
use super::cursor::{Reader, Writer};

pub(crate) fn encode_value(w: &mut Writer, value: &Value) {
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
            w.u16(items.len() as u16);
            for item in items {
                encode_value(w, item);
            }
        }
        Value::Record(fields) => {
            w.u16(fields.len() as u16);
            for (index, val) in fields {
                w.u16(*index);
                encode_value(w, val);
            }
        }
        // `Value` is `#[non_exhaustive]`; an unknown variant cannot be encoded
        // without a tag, so it is skipped. The value codec is exercised only
        // on values lowered by the (known) type checker.
        _ => {}
    }
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
    encode_value(&mut w, value);
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

pub(crate) fn decode_value(r: &mut Reader<'_>) -> Result<Value, WireError> {
    let tag = r.u8("value.tag")?;
    match tag {
        TAG_NULL => Ok(Value::Null),
        TAG_INT => Ok(Value::Int(r.i64("value.int")?)),
        TAG_FLOAT => Ok(Value::Float(f64::from_bits(r.u64("value.float")?))),
        TAG_BOOL => Ok(Value::Bool(r.u8("value.bool")? != 0)),
        TAG_STR => Ok(Value::Str(r.u32("value.str")?)),
        TAG_HANDLER => Ok(Value::HandlerRef(r.u32("value.handler")?)),
        TAG_LIST => {
            let count = r.u16("value.list.count")?;
            r.ensure_capacity(count as usize, "value.list")?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(decode_value(r)?);
            }
            Ok(Value::List(items))
        }
        TAG_RECORD => {
            let count = r.u16("value.record.count")?;
            r.ensure_capacity(count as usize, "value.record")?;
            let mut fields = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let index = r.u16("value.record.index")?;
                let val = decode_value(r)?;
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
