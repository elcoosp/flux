//! `Props` wire codec (Appendix D §D.3 props section).

use flux_syntax::Props;

use super::cursor::Reader;
use super::value::encode_value;
use super::{WireError, decode_value};

pub(crate) fn encode_props(w: &mut super::cursor::Writer, props: &Props) {
    w.u16(props.fields().len() as u16);
    for (index, value) in props.fields() {
        w.u16(*index);
        encode_value(w, value);
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
