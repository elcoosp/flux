//! `StringEntry` wire codec (Appendix D §D.9).

use flux_syntax::StringId;

use super::core::WireError;
use super::cursor::{Reader, Writer};

pub(crate) fn encode_string_entry(w: &mut Writer, id: StringId, text: &str) {
    w.u32(id);
    w.u16_len(text.len(), "string_entry.text");
    w.bytes(text.as_bytes());
}

pub(crate) fn decode_string_entry(r: &mut Reader<'_>) -> Result<(StringId, String), WireError> {
    let id = r.u32("string.id")?;
    let len = r.u16("string.len")? as usize;
    let raw = r.bytes(len, "string.bytes")?;
    let text = std::str::from_utf8(raw)
        .map_err(|_| WireError::InvalidUtf8 {
            context: "string",
            at: r.pos() - len,
        })?
        .to_owned();
    Ok((id, text))
}
