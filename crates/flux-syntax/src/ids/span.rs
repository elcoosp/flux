#![allow(clippy::module_inception)] // submodule named for its single responsibility (Span lives in 'span').
use super::fnv::*;
use serde::{Deserialize, Serialize};
///
/// let span = Span::new(0, 10, 20);
/// assert!(span.contains(10));
/// assert!(!span.contains(20));
/// assert_eq!(span.len(), 10);
/// ```
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Span {
    /// Source file this span points into.
    pub file_id: FileId,
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
}

impl Span {
    /// Creates a span covering `start..end` in `file_id`.
    #[must_use]
    pub const fn new(file_id: FileId, start: u32, end: u32) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }

    /// Returns the 1-based line/column of `byte` within `source`, counting
    /// newlines before `byte`. Used to turn a byte span into `path:line:col`
    /// for on-device diagnostics without shipping the whole source file.
    #[must_use]
    pub fn line_col_of(source: &str, byte: u32) -> (u16, u16) {
        let mut line: u16 = 1;
        let mut col: u16 = 1;
        for (i, ch) in source.char_indices() {
            if (i as u32) >= byte {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }
}

/// A server-computed source excerpt for off-device diagnostics (ADR-0057).
///
/// Carried on each `ClosureRef` (so a VM fault maps `offset → handler →
/// snippet` offline) and on the `Error` frame (so a compile/type error ships
/// `path:line:col` + the offending line). The host resolves `file_id` to a path
/// via the frame's `source_map` and renders `snippet` with a `^` caret at
/// `col`. Computed once at compile time from the source text the server already
/// holds; never re-derived on the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceExcerpt {
    /// Source file the excerpt points into (resolves to a path via `source_map`).
    pub file_id: FileId,
    /// Inclusive start byte offset of the cited span.
    pub byte_start: u32,
    /// Exclusive end byte offset of the cited span.
    pub byte_end: u32,
    /// 1-based line of `byte_start`.
    pub line: u16,
    /// 1-based column of `byte_start`.
    pub col: u16,
    /// The cited source line, trimmed of leading/trailing whitespace.
    pub snippet: String,
}

impl SourceExcerpt {
    /// Builds an excerpt from a [`Span`] and the source text it points into.
    ///
    /// Returns `None` when `file_id` has no text (e.g. runtime-generated trees),
    /// so the wire field is genuinely optional and the host degrades to a span
    /// without a snippet. The cited line is the one containing `span.start`.
    #[must_use]
    pub fn from_span(span: Span, source: &str) -> Option<SourceExcerpt> {
        if source.is_empty() {
            return None;
        }
        let (line, col) = Span::line_col_of(source, span.start);
        // The cited line is the slice from the last newline before `start` to
        // the next newline after `start`.
        let start = span.start as usize;
        let line_start = source[..start.min(source.len())]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = source[start.min(source.len())..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(source.len());
        let snippet = source[line_start..line_end.min(source.len())]
            .trim()
            .to_owned();
        Some(SourceExcerpt {
            file_id: span.file_id,
            byte_start: span.start,
            byte_end: span.end,
            line,
            col,
            snippet,
        })
    }
}

impl Span {
    /// Returns the length of the span in bytes, saturating at zero for
    /// malformed (inverted) spans.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` when the span covers no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` when `offset` lies within `start..end`.
    #[must_use]
    pub const fn contains(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Returns the smallest span covering both `self` and `other`.
    ///
    /// The file of `self` wins when the two spans come from different files;
    /// callers join spans only within a single file during parsing, so a
    /// mismatch means a bug upstream rather than a recoverable condition.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        Self {
            file_id: self.file_id,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
