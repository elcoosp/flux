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
pub(crate) fn encode_str(w: &mut Writer, s: &str) -> Result<(), WireError> {
    // FLUX-F57: use the version-dispatched length prefix, matching
    // `decode_str` below. The previous encoder wrote a u16 length
    // unconditionally while `decode_str` reads u32 at v3 (via
    // `Reader::count`), so any `ClosureRef.excerpt.snippet` on a v3
    // frame desynced the decoder — it interpreted the 2-byte length
    // plus the first two snippet bytes as a 4-byte u32 and failed with
    // a "truncated frame" error far past the actual buffer end.
    //
    // The frame encoders in `frame.rs` already use the
    // version-dispatched `Writer::count_prefix`; this helper was the
    // one outlier.
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
