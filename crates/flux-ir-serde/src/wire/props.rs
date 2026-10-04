//! `Props` wire codec (Appendix D §D.3 props section).

use flux_syntax::Props;

use super::cursor::Reader;
use super::value::encode_value;
use super::{WireError, decode_value};

pub(crate) fn encode_props(
    w: &mut super::cursor::Writer,
    props: &Props,
) -> Result<(), WireError> {
    // Audit H14 cascade: fallible now, so a value that overflows the u16
    // prefix propagates through `encode_node` → `encode_patch` → the frame
    // encoders rather than panicking mid-write. The caller still `.expect()`s
    // at the frame boundary pending the top-level `try_to_bytes` migration.
    w.u16_len_checked(props.fields().len(), "props.fields")?;
    for (index, value) in props.fields() {
        w.u16(*index);
        encode_value(w, value)?;
    }
    Ok(())
}

pub(crate) fn decode_props(r: &mut Reader<'_>) -> Result<Props, WireError> {
    let count = r.u16("props.count")?;
    r.ensure_capacity(count as usize, "props")?;
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let index = r.u16("props.index")?;
        let value = decode_value(r)?;
        fields.push((index, value));
    }
    Ok(Props::from_fields(fields))
}
