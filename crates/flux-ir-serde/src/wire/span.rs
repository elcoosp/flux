//! `Span` and length-prefixed UTF-8 string wire helpers (Appendix D §D.3).

use flux_syntax::Span;

use super::core::WireError;
use super::cursor::{Reader, Writer};

pub fn encode_span(w: &mut Writer, span: &Span) {
    w.u32(span.file_id);
    w.u32(span.start);
    w.u32(span.end);
}

pub fn decode_span(r: &mut Reader<'_>) -> Result<Span, WireError> {
    let file_id = r.u32("span.file")?;
    let start = r.u32("span.start")?;
    let end = r.u32("span.end")?;
    Ok(Span::new(file_id, start, end))
}

/// Writes a length-prefixed UTF-8 string (u16 byte length + bytes), matching the
/// layout `frame::encode_str` uses for `Error`/`Hello` payloads.
pub(crate) fn encode_str(w: &mut Writer, s: &str) {
    w.u16(s.len() as u16);
    w.bytes(s.as_bytes());
}

/// Reads a length-prefixed UTF-8 string (u16 byte length + bytes).
pub(crate) fn decode_str(r: &mut Reader<'_>, ctx: &'static str) -> Result<String, WireError> {
    let len = r.u16(ctx)? as usize;
    let raw = r.bytes(len, ctx)?;
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| WireError::InvalidUtf8 {
            context: ctx,
            at: r.pos(),
        })
}
