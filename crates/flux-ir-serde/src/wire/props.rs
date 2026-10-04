//! `Props` wire codec (Appendix D §D.3 props section).

use flux_syntax::Props;

use super::cursor::Reader;
use super::value::encode_value;
use super::{WireError, decode_value};

pub(crate) fn encode_props(w: &mut super::cursor::Writer, props: &Props) {
    w.u16_len(props.fields().len(), "props.fields");
    for (index, value) in props.fields() {
        w.u16(*index);
        // Scaffold: `encode_props` still returns `()`, so a length-overflow
        // in a nested `Value` panics with a clear context. Migrating
        // `encode_props` to `Result` is a follow-up that threads `?` up
        // through `encode_node` → frame encoders (audit H14 cascade).
        encode_value(w, value).expect("props: value exceeds wire length limits");
    }
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
