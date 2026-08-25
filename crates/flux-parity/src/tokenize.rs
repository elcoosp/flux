//! Shared tokenizer for the release-path recognizers ([`crate::recognize_swift`],
//! [`crate::recognize_kotlin`]).
//!
//! Both backends emit a deterministic, brace/paren-delimited surface, so a single
//! line-oriented tokenizer plus delimiter matchers is enough to recover the
//! structural [`crate::model::ViewNode`] tree from the emitted Swift/Kotlin source.

/// A lexical token carrying its source line (for diagnostics).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// The token text (identifier, string, number, or a single delimiter).
    pub text: String,
    /// Zero-based source line the token came from.
    pub line: usize,
}

/// Tokenizes `src` line-by-line into [`Token`]s.
pub(crate) fn tokenize(src: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    for (line, raw) in src.split('\n').enumerate() {
        for tok in split_tokens(raw.trim()) {
            tokens.push(Token { text: tok, line });
        }
    }
    tokens
}

/// Splits one line into tokens, keeping braces, parens, commas and `:` as
/// standalone tokens while preserving identifiers/strings/number runs.
fn split_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            buf.push(ch);
            for c in chars.by_ref() {
                buf.push(c);
                if c == '"' {
                    break;
                }
            }
            continue;
        }
        if "{}():,".contains(ch) {
            if !buf.is_empty() {
                out.push(std::mem::take(&mut buf));
            }
            out.push(ch.to_string());
            continue;
        }
        if ch.is_whitespace() {
            if !buf.is_empty() {
                out.push(std::mem::take(&mut buf));
            }
            continue;
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

/// Returns the index of the `}` matching the `{` at `open`.
pub(crate) fn match_brace(tokens: &[Token], open: usize) -> Option<usize> {
    match_delim(tokens, open, '{', '}')
}

/// Returns the index of the `)` matching the `(` at `open`.
pub(crate) fn match_paren(tokens: &[Token], open: usize) -> Option<usize> {
    match_delim(tokens, open, '(', ')')
}

/// Returns the index of the closing delimiter matching the opening delimiter at
/// `open`, tracking nesting depth.
fn match_delim(tokens: &[Token], open: usize, open_ch: char, close_ch: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    while i < tokens.len() {
        match tokens[i].text.as_str() {
            s if s.starts_with(open_ch) => depth += 1,
            s if s.starts_with(close_ch) => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
