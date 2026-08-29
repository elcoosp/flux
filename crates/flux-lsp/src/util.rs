//! Shared helpers for translating between LSP positions and Flux byte spans.
//!
//! Flux source is UTF-8; the LSP column convention is also UTF-8 for the
//! `flux-lsp` server (the Flux surface grammar is ASCII for identifiers and
//! keywords, so a UTF-8 column equals the editor's column in practice). All
//! conversions are pure functions of the document text.

use async_lsp::lsp_types::{Position, Range};
use flux_syntax::Span;

/// Converts a 0-based `(line, character)` LSP position into a 0-based byte
/// offset within `text`, or `None` if the position is past the end of the file.
///
/// Counts `line` by scanning for `\n` boundaries and `character` by counting
/// UTF-8 bytes within the target line (Flux identifiers are ASCII, so this
/// matches the editor column). Returns `None` only for a line past EOF; an
/// in-range-but-truncated column clamps to the end of that line.
#[must_use]
pub(crate) fn position_to_offset(text: &str, line: u32, character: u32) -> Option<u32> {
    let mut current_line: u32 = 0;
    let mut line_start: u32 = 0;
    for (idx, ch) in text.char_indices() {
        if current_line == line {
            let col = (idx - line_start as usize) as u32;
            if col >= character {
                return Some(idx as u32);
            }
        }
        if ch == '\n' {
            current_line += 1;
            line_start = (idx + 1) as u32;
        }
    }
    // Past the final newline: if we reached/just-passed `line`, clamp to end.
    if current_line >= line {
        return Some(text.len() as u32);
    }
    None
}

/// Converts a byte [`Span`] into an LSP [`Range`] (0-based line/column).
///
/// Returns a zero-width range when `span` is empty so the editor can still
/// place a cursor at the definition.
#[must_use]
pub(crate) fn span_to_range(text: &str, span: Span) -> Range {
    let start = offset_to_position(text, span.start);
    let end = if span.is_empty() {
        start
    } else {
        offset_to_position(text, span.end)
    };
    Range { start, end }
}

/// Converts a 0-based byte `offset` into an LSP [`Position`].
#[must_use]
pub(crate) fn offset_to_position(text: &str, offset: u32) -> Position {
    let mut line: u32 = 0;
    let mut line_start: u32 = 0;
    for (idx, ch) in text.char_indices() {
        if idx as u32 >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = (idx + 1) as u32;
        }
    }
    Position {
        line,
        character: offset.saturating_sub(line_start),
    }
}

/// Applies one incremental LSP content change (with a `range`) to `text`,
/// returning the updated document, or `None` if the range cannot be mapped
/// (so the caller can fall back to a full replace).
///
/// LSP ranges are 0-based line/column; Flux source is UTF-8, so the byte
/// offsets are derived via [`position_to_offset`] (ASCII columns equal byte
/// columns for the Flux surface grammar). The replaced span `[start, end)` is
/// swapped for `replacement`.
#[must_use]
pub(crate) fn apply_range_edit(
    text: &str,
    range: async_lsp::lsp_types::Range,
    replacement: &str,
) -> Option<String> {
    let start = position_to_offset(text, range.start.line, range.start.character)? as usize;
    let end = position_to_offset(text, range.end.line, range.end.character)? as usize;
    if start > end || end > text.len() {
        return None;
    }
    let mut updated = String::with_capacity(text.len() + replacement.len());
    updated.push_str(&text[..start]);
    updated.push_str(replacement);
    updated.push_str(&text[end..]);
    Some(updated)
}
