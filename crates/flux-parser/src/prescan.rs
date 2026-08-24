//! A lexical pre-scan that produces better diagnostics than backtracking can.
//!
//! pest backtracks out of a partially-matched rule, so an unterminated string
//! or an unclosed brace surfaces as a generic failure at end of file. Scanning
//! for those two conditions first lets the parser point at the *opening* token,
//! which is what makes the diagnostic actionable (AGENTS.md §3.7).

use flux_syntax::Span;

use crate::error::ParseError;
use crate::lower::Ctx;

/// Maximum block nesting depth accepted by the parser.
///
/// pest's generated parser descends the whole expression-precedence chain at
/// every nesting level, costing roughly 100 KB of stack per level, so
/// unbounded nesting aborts the process instead of producing a diagnostic.
/// The limit is checked lexically before parsing so deep input fails with an
/// actionable error on any thread, including the ~2 MB stacks test harnesses
/// use. The value was chosen by measuring the depth that parses safely there;
/// real view trees nest far less. Recorded as extension G6 in
/// `/docs/adr/parser-grammar-extensions.md`.
pub(crate) const MAX_NESTING_DEPTH: usize = 16;

/// Reports the first unterminated string literal or unclosed brace in `source`.
///
/// Returns `None` when the source is lexically balanced, in which case pest's
/// own error is the more precise one.
pub(crate) fn prescan(ctx: &Ctx<'_>) -> Option<ParseError> {
    scan(ctx, ScanMode::Balance)
}

/// Rejects input nested deeper than [`MAX_NESTING_DEPTH`].
pub(crate) fn check_depth(ctx: &Ctx<'_>) -> Option<ParseError> {
    scan(ctx, ScanMode::Depth)
}

/// Which condition a scan reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanMode {
    /// Report unterminated strings and unclosed braces.
    Balance,
    /// Report excessive nesting depth.
    Depth,
}

fn scan(ctx: &Ctx<'_>, mode: ScanMode) -> Option<ParseError> {
    let mut braces: Vec<usize> = Vec::new();
    let mut bytes = ctx.source.char_indices().peekable();
    while let Some((offset, character)) = bytes.next() {
        match character {
            '/' if matches!(bytes.peek(), Some((_, '/'))) => {
                for (_, next) in bytes.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                if !skip_string(&mut bytes) {
                    return match mode {
                        ScanMode::Balance => Some(unterminated_string(ctx, offset)),
                        ScanMode::Depth => None,
                    };
                }
            }
            '{' => {
                braces.push(offset);
                if mode == ScanMode::Depth && braces.len() > MAX_NESTING_DEPTH {
                    return Some(too_deep(ctx, offset));
                }
            }
            '}' => {
                // An unmatched `}` is pest's error to report precisely; the
                // scan cannot say anything useful about the rest of the file.
                braces.pop()?;
            }
            _ => {}
        }
    }
    match mode {
        ScanMode::Balance => braces.first().map(|offset| unclosed_brace(ctx, *offset)),
        ScanMode::Depth => None,
    }
}

/// Consumes a string literal body, returning `false` when it never closes.
///
/// Interpolations may themselves contain braces and nested strings, so the
/// scan tracks interpolation depth while looking for the closing quote.
fn skip_string<I>(bytes: &mut std::iter::Peekable<I>) -> bool
where
    I: Iterator<Item = (usize, char)>,
{
    let mut interp_depth = 0usize;
    while let Some((_, character)) = bytes.next() {
        match character {
            '\\' => {
                if bytes.next().is_none() {
                    return false;
                }
            }
            '{' => interp_depth += 1,
            '}' => interp_depth = interp_depth.saturating_sub(1),
            '"' if interp_depth == 0 => return true,
            '\n' => return false,
            _ => {}
        }
    }
    false
}

fn unterminated_string(ctx: &Ctx<'_>, offset: usize) -> ParseError {
    let end = ctx.source[offset..]
        .find('\n')
        .map_or(ctx.source.len(), |index| offset + index);
    ctx.error(
        Span::new(ctx.file_id, offset as u32, end as u32),
        "unterminated string literal".to_owned(),
        Some(
            "the literal opens here and no closing `\"` follows before the end \
             of the line — add one, or escape an intended quote as `\\\"`"
                .to_owned(),
        ),
    )
}

fn unclosed_brace(ctx: &Ctx<'_>, offset: usize) -> ParseError {
    ctx.error(
        Span::new(ctx.file_id, offset as u32, offset as u32 + 1),
        "unclosed `{`".to_owned(),
        Some("this block is never closed — add the matching `}`".to_owned()),
    )
}

fn too_deep(ctx: &Ctx<'_>, offset: usize) -> ParseError {
    ctx.error(
        Span::new(ctx.file_id, offset as u32, offset as u32 + 1),
        format!("block nesting exceeds the maximum depth of {MAX_NESTING_DEPTH}"),
        Some(
            "extract the inner view into its own `component` — deeply nested \
             trees are also slower to diff and harder to read"
                .to_owned(),
        ),
    )
}
