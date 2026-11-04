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

/// Writes a length-prefixed UTF-8 string, matching the layout
/// [`decode_str`] reads: the prefix width follows the writer's protocol
/// version (v3 `u32`, v2 `u16`, ADR-0059).
///
/// A fixed `u16` prefix here desynced every v3 frame carrying a non-empty
/// string: the decoder read a `u32` assembled from the `u16` length and the
/// first two payload bytes (round-21, ADR-0057 ClosureRef excerpts).
pub(crate) fn encode_str(w: &mut Writer, s: &str) -> Result<(), WireError> {
    // Fallible length prefix so an oversized excerpt surfaces as a
    // recoverable `WireError` at this encoder's boundary, not a panic
    // (audit H14).
    w.count_prefix(s.len(), "span.len")?;
    w.bytes(s.as_bytes());
    Ok(())
}

/// Reads a length-prefixed UTF-8 string (u16 byte length + bytes).
pub(crate) fn decode_str(r: &mut Reader<'_>, ctx: &'static str) -> Result<String, WireError> {
    // ADR-0059: the length prefix width depends on `r`'s protocol version.
    // `Reader::count` returns u32 either way (u16 for v2 is widened).
    let len = r.count(ctx)? as usize;
    let raw = r.bytes(len, ctx)?;
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| WireError::InvalidUtf8 {
            context: ctx,
            at: r.pos(),
        })
}
