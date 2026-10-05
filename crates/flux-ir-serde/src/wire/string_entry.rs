//! `StringEntry` wire codec (Appendix D §D.9).

use flux_syntax::StringId;

use super::core::WireError;
use super::cursor::{Reader, Writer};

pub(crate) fn encode_string_entry(
    w: &mut Writer,
    id: StringId,
    text: &str,
) -> Result<(), WireError> {
    // Audit fix (H14): a single interned string larger than 64 KB (a
    // scraped blob, a very long asset path) now surfaces as a typed
    // `WireError::LengthExceedsU16` at this boundary instead of panicking
    // inside `u16_len`. Callers must propagate the result.
    w.u32(id);
    w.count_prefix(text.len(), "string_entry.text")?;
    w.bytes(text.as_bytes());
    Ok(())
}

pub(crate) fn decode_string_entry(r: &mut Reader<'_>) -> Result<(StringId, String), WireError> {
    let id = r.u32("string.id")?;
    let len = r.count("string.len")? as usize;
    let raw = r.bytes(len, "string.bytes")?;
    let text = std::str::from_utf8(raw)
        .map_err(|_| WireError::InvalidUtf8 {
            context: "string",
            at: r.pos() - len,
        })?
        .to_owned();
    Ok((id, text))
}
