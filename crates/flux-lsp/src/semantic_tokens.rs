//! Compiler-driven semantic highlighting for `.flux` (FLUX-024 / PRD-O user
//! story 1).
//!
//! The provider reuses the real lexer ([`flux_parser::tokenize`]) so highlight
//! token types always match the surface grammar the parser accepts — there is
//! no second regex grammar to drift. Layout tokens (`Indent`/`Dedent`/`Newline`)
//! and bare identifiers carry no highlight; line comments are recovered with a
//! small dedicated scan because the lexer discards them.
//!
//! Tokens are emitted in LSP relative-coordinate form (delta-line / delta-start),
//! exactly as `lsp_types::SemanticToken` expects.
//!
//! Scope note: highlighting is lexer-driven, so it classifies keywords, string
//! and number literals, and comments. Type names, `null`/`None`/`Some`, and
//! component/prop names are syntactically plain identifiers to the lexer; a
//! parser/AST-aware pass (FLUX-027) is needed to highlight those. The legend
//! therefore advertises only the four token types the lexer can produce.

use async_lsp::lsp_types::{SemanticToken, SemanticTokenType, SemanticTokensLegend};
use flux_parser::TokenKind;

/// The token-type vocabulary advertised in [`legend`].
///
/// Indices into this vector are the `token_type` field of every emitted
/// [`SemanticToken`]; keep them stable.
const KEYWORD: u32 = 0;
const STRING: u32 = 1;
const NUMBER: u32 = 2;
const COMMENT: u32 = 3;

/// The semantic-tokens legend: the ordered token types the server emits.
#[must_use]
pub(crate) fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::COMMENT,
        ],
        token_modifiers: Vec::new(),
    }
}

/// Maps a lexer token kind to a highlight type index, or `None` when the token
/// carries no highlight (operators, punctuation, layout, plain identifiers, and
/// type/value names the lexer cannot distinguish from identifiers).
#[must_use]
pub(crate) fn classify(kind: TokenKind) -> Option<u32> {
    use TokenKind::*;
    Some(match kind {
        // Declaration keywords.
        Compo | Type | Trait | Capability | State | Derived | Fn | Let | Use | Import => KEYWORD,
        // Control / lifecycle keywords.
        If | Else | When | Otherwise | Match | Effect | OnMount | OnCleanup | Batch | Untrack
        | Resource | Await | Provide | UseContext | CreateRef => KEYWORD,
        // The boolean literals `true` / `false` are lexed as `Bool`.
        Bool => KEYWORD,
        // Literals.
        Str => STRING,
        Int | Float => NUMBER,
        // Everything else (operators, punctuation, layout, bare idents) is
        // left un-highlighted.
        _ => return None,
    })
}

/// Computes the semantic-token stream for `src` in LSP relative coordinates.
///
/// Line comments are recovered with [`comment_spans`] (the lexer discards them);
/// all other tokens come from the real lexer so highlighting never disagrees
/// with the parser.
#[must_use]
pub(crate) fn tokens_for_text(src: &str) -> Vec<SemanticToken> {
    let mut raw: Vec<(usize, usize, u32)> = Vec::new();

    if let Ok(tokens) = flux_parser::tokenize(src) {
        for token in tokens {
            if let Some(idx) = classify(token.kind) {
                raw.push((token.start, token.end, idx));
            }
        }
    }
    for (start, end) in comment_spans(src) {
        raw.push((start, end, COMMENT));
    }

    // Relative encoding requires tokens sorted by start offset.
    raw.sort_by_key(|(start, _, _)| *start);

    let mut out = Vec::with_capacity(raw.len());
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;
    for (start, end, idx) in raw {
        let (line, character) = line_col_at(src, start);
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 {
            character - prev_char
        } else {
            character
        };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: (end.saturating_sub(start)) as u32,
            token_type: idx,
            token_modifiers_bitset: 0,
        });
        prev_line = line;
        prev_char = character;
    }
    out
}

/// Returns `(0-based line, 0-based character)` for `byte` in `src`, counting
/// characters (not bytes) for the column so multi-byte source aligns.
fn line_col_at(src: &str, byte: usize) -> (u32, u32) {
    let byte = byte.min(src.len());
    let before = &src[..byte];
    let line = before.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let character = src[line_start..byte].chars().count() as u32;
    (line, character)
}

/// Recovers `//` line-comment spans, ignoring `//` that appears inside a string
/// literal (a `//` in `"http://"` must not be highlighted as a comment).
fn comment_spans(src: &str) -> Vec<(usize, usize)> {
    let bytes = src.as_bytes();
    let mut spans = Vec::new();
    let mut in_string = false;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                if i == 0 || bytes[i - 1] != b'\\' {
                    in_string = !in_string;
                }
                i += 1;
            }
            b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                let line_end = src[i..].find('\n').map_or(src.len(), |d| i + d);
                spans.push((i, line_end));
                i = if line_end > i { line_end } else { i + 1 };
            }
            _ => i += 1,
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_strings_numbers_and_comments_are_classified() {
        let src =
            "compo Counter\n    $count: Int = 0\n    Text text: \"hi {count}\"\n    // note\n";
        let tokens = tokens_for_text(src);
        let mut kinds = tokens.iter().map(|t| t.token_type).collect::<Vec<_>>();
        kinds.sort_unstable();
        assert!(kinds.contains(&KEYWORD), "expected a keyword highlight");
        assert!(kinds.contains(&STRING), "expected a string highlight");
        assert!(kinds.contains(&NUMBER), "expected a number highlight");
        assert!(kinds.contains(&COMMENT), "expected a comment highlight");
    }

    #[test]
    fn relative_encoding_is_monotonic_and_non_negative() {
        let src = "compo A\n    let x: Int = 1\n    // note\n";
        let tokens = tokens_for_text(src);
        let mut prev_line = 0i64;
        let mut prev_char = 0i64;
        for t in &tokens {
            let line = prev_line + i64::from(t.delta_line);
            let character = if t.delta_line == 0 {
                prev_char + i64::from(t.delta_start)
            } else {
                i64::from(t.delta_start)
            };
            assert!(t.delta_line <= 2, "delta_line unexpectedly large");
            assert!(t.length > 0, "zero-length token");
            prev_line = line;
            prev_char = character;
        }
    }

    #[test]
    fn http_url_inside_string_is_not_a_comment() {
        let src = "let u: String = \"http://example.com\"\n";
        let tokens = tokens_for_text(src);
        // A single string token; no comment token (index COMMENT) should be
        // produced for the in-string `//`.
        assert!(
            !tokens.iter().any(|t| t.token_type == COMMENT),
            "in-string // must not be highlighted as a comment"
        );
    }
}
