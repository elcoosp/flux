//! `ClosureRef` wire codec (Appendix D §D.7).

use flux_syntax::{ClosureRef, FileId, SourceExcerpt};

use super::WireError;
use super::cursor::Reader;
use super::span::{decode_span, encode_span, encode_str};

pub(crate) fn encode_closure_ref(w: &mut super::cursor::Writer, closure: &ClosureRef) {
    w.u64(closure.hash);
    w.u32(closure.bytecode_offset);
    w.u16(closure.bytecode_len);
    w.u16(closure.captured_signals.len() as u16);
    for signal in &closure.captured_signals {
        w.u32(*signal);
    }
    encode_span(w, &closure.span);
    // ADR-0057: trailing server-computed source excerpt (gated by `has`), so a
    // VM fault maps `offset → handler → path:line:col + snippet` offline. Absent
    // on v1-derived trees (no source text) and decode-skipped there.
    match &closure.excerpt {
        Some(ex) => {
            w.u8(1);
            w.u32(ex.file_id);
            w.u32(ex.byte_start);
            w.u32(ex.byte_end);
            w.u16(ex.line);
            w.u16(ex.col);
            encode_str(w, &ex.snippet);
        }
        None => w.u8(0),
    }
}

pub(crate) fn decode_closure_ref(r: &mut Reader<'_>) -> Result<ClosureRef, WireError> {
    let hash = r.u64("closure.hash")?;
    let bytecode_offset = r.u32("closure.offset")?;
    let bytecode_len = r.u16("closure.len")?;
    let signal_count = r.u16("closure.signal_count")?;
    r.ensure_capacity(signal_count as usize, "closure.signals")?;
    let mut captured_signals = Vec::with_capacity(signal_count as usize);
    for _ in 0..signal_count {
        captured_signals.push(r.u32("closure.signal")?);
    }
    let span = decode_span(r)?;
    let excerpt = if r.u8("closure.excerpt.present")? != 0 {
        Some(SourceExcerpt {
            file_id: FileId::from(r.u32("closure.excerpt.file")?),
            byte_start: r.u32("closure.excerpt.start")?,
            byte_end: r.u32("closure.excerpt.end")?,
            line: r.u16("closure.excerpt.line")?,
            col: r.u16("closure.excerpt.col")?,
            snippet: super::span::decode_str(r, "closure.excerpt.snippet")?,
        })
    } else {
        None
    };
    Ok(ClosureRef {
        hash,
        bytecode_offset,
        bytecode_len,
        captured_signals,
        span,
        excerpt,
    })
}
