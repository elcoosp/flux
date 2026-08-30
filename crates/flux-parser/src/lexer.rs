//! The Flux lexical analyzer.
//!
//! The lexer performs a single left-to-right pass over the source, producing a
//! flat `Vec<Token>` with byte spans. It is allocation-free per token: tokens
//! carry byte ranges into the source, and the parser slices the text it needs.
//!
//! Two features make it suitable for the indentation-delimited surface syntax
//! (Appendix B as revised by the FLUX-00X syntax ADR):
//!
//! * **INDENT / DEDENT layout tokens.** At brace depth zero, line boundaries
//!   carry layout meaning. The lexer compares the indentation of the next
//!   significant line against a stack of open indent levels and emits `Indent`,
//!   `Dedent` or `Newline` so the parser can delimit component bodies and view
//!   children without braces. Inside `( [ {` the newline is ordinary whitespace
//!   and emits nothing.
//! * **`||` vs `|`.** A doubled pipe is the boolean-or operator *and* the
//!   parameterless-lambda marker; a single pipe is the lambda/branch marker.
//!   The parser disambiguates by position, so the lexer keeps them distinct.
//!
//! A 500-line file lexes in well under the 5 ms parse budget (Appendix C §3.6).

use flux_syntax::Span;

/// A lexical token kind.
///
/// The discriminant values are stable for the lifetime of the surface grammar;
/// downstream consumers (the `flux-lsp` semantic-tokens provider) match on the
/// variant, not the numeric value. Layout tokens (`Indent`/`Dedent`/`Newline`)
/// carry no highlights of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// Layout: indentation increased relative to the enclosing level.
    Indent,
    /// Layout: indentation decreased; closes one or more blocks.
    Dedent,
    /// Layout: a line boundary at the same indent as the enclosing block.
    Newline,

    /// An integer literal (may carry a leading `-`).
    Int,
    /// A floating-point literal.
    Float,
    /// A string literal, including `{interp}` segments.
    Str,
    /// A boolean literal `true` / `false`.
    Bool,

    /// An identifier (not a keyword), or a `$name` state sigil.
    Ident,

    /// `compo` / `component` — a component declaration.
    Compo,
    /// `fn` — a function or lambda header.
    Fn,
    /// `let` — a binding.
    Let,
    /// `if`.
    If,
    /// `else`.
    Else,
    /// `when`.
    When,
    /// `otherwise`.
    Otherwise,
    /// `match`.
    Match,
    /// `use`.
    Use,
    /// `type`.
    Type,
    /// `record` — a product-type (struct) declaration.
    Record,
    /// `trait`.
    Trait,
    /// `capability`.
    Capability,
    /// `state` — the legacy state keyword (the `$` sigil is preferred).
    State,
    /// `derived`.
    Derived,
    /// `effect`.
    Effect,
    /// `onMount`.
    OnMount,
    /// `onCleanup`.
    OnCleanup,
    /// `batch`.
    Batch,
    /// `untrack`.
    Untrack,
    /// `resource`.
    Resource,
    /// `await`.
    Await,
    /// `createRef`.
    CreateRef,
    /// `provide`.
    Provide,
    /// `useContext`.
    UseContext,

    /// `:` — a prop/field separator.
    Colon,
    /// `,` — argument / entry separator.
    Comma,
    /// `=` — assignment, not `==`.
    Eq,
    /// `==` — equality comparison.
    EqEq,
    /// `!=` — inequality comparison.
    NotEq,
    /// `!` — boolean negation prefix (desugared to `!= true` by the parser).
    Not,
    /// `<` — less-than.
    Lt,
    /// `>` — greater-than.
    Gt,
    /// `<=` — less-than-or-equal.
    LtEq,
    /// `>=` — greater-than-or-equal.
    GtEq,
    /// `=>` — a branch or closure-parameter arrow.
    FatArrow,
    /// `.` — a field or member access.
    Dot,
    /// `?.` — optional (null-safe) member access (FLUX-053 / ADR-0051).
    QuestionDot,
    /// `->` — a function return-type arrow.
    Arrow,
    /// `@` — an annotation marker.
    At,
    /// `(` — open parenthesis.
    LParen,
    /// `)` — close parenthesis.
    RParen,
    /// `{` — open brace.
    LBrace,
    /// `}` — close brace.
    RBrace,
    /// `[` — open bracket.
    LBracket,
    /// `]` — close bracket.
    RBracket,
    /// `+` — addition.
    Plus,
    /// `-` — subtraction (binary) or the sign of a number literal.
    Minus,
    /// `*` — multiplication.
    Star,
    /// `/` — division.
    Slash,
    /// `%` — remainder.
    Percent,
    /// `&` — boolean and.
    And,
    /// `|` — lambda/branch marker.
    Pipe,
    /// `||` — boolean-or *and* parameterless-lambda marker.
    Or,
    /// `...` — an elided body marker.
    Ellipsis,

    /// End of input.
    Eof,
}

/// One lexical token: its kind and the byte span it covers in the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// Token classification.
    pub kind: TokenKind,
    /// Inclusive-start byte offset.
    pub start: usize,
    /// Exclusive-end byte offset.
    pub end: usize,
}

impl Token {
    /// Builds a span for this token in `file_id`.
    #[must_use]
    pub(crate) fn span(self, file_id: u32) -> Span {
        Span::new(file_id, self.start as u32, self.end as u32)
    }
}

/// A lexical error with a byte span for diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    /// Human-readable description.
    pub(crate) message: String,
    /// Byte span of the offending text.
    pub(crate) span: Span,
}

impl LexError {
    fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

/// Maps an identifier's text to its keyword kind, if it is a keyword.
///
/// Returns `None` for identifiers and the `$name` state sigil.
#[must_use]
pub fn keyword_kind(text: &str) -> Option<TokenKind> {
    Some(match text {
        "compo" | "component" => TokenKind::Compo,
        "fn" => TokenKind::Fn,
        "let" => TokenKind::Let,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "when" => TokenKind::When,
        "otherwise" => TokenKind::Otherwise,
        "match" => TokenKind::Match,
        "use" => TokenKind::Use,
        "type" => TokenKind::Type,
        "record" => TokenKind::Record,
        "trait" => TokenKind::Trait,
        "capability" => TokenKind::Capability,
        "state" => TokenKind::State,
        "derived" => TokenKind::Derived,
        "effect" => TokenKind::Effect,
        "onMount" => TokenKind::OnMount,
        "onCleanup" => TokenKind::OnCleanup,
        "batch" => TokenKind::Batch,
        "untrack" => TokenKind::Untrack,
        "resource" => TokenKind::Resource,
        "await" => TokenKind::Await,
        "createRef" => TokenKind::CreateRef,
        "provide" => TokenKind::Provide,
        "useContext" => TokenKind::UseContext,
        "true" | "false" => TokenKind::Bool,
        _ => return None,
    })
}

/// Lexes `source` into tokens, resolving indentation into layout tokens.
///
/// The returned tokens carry byte spans; callers that only need highlighting
/// (e.g. the `flux-lsp` semantic-tokens provider) can iterate `(kind, start,
/// end)` without involving the parser. A `LexError` is returned for an
/// unterminated string literal or a dedent that matches no enclosing indent.
///
/// # Errors
///
/// Returns a [`LexError`] for an unterminated string literal or a dedent that
/// matches no enclosing indent level.
pub fn lex(source: &str, file_id: u32) -> Result<Vec<Token>, LexError> {
    let lexer = Lexer::new(source, file_id);
    lexer.run()
}

/// Lexes `source` for a highlight-only consumer, using synthetic `file_id` `0`.
///
/// Unlike [`parse`](crate::parse) this never builds an `Ast`; it returns the raw
/// token stream with byte spans so a client (such as the LSP semantic-tokens
/// provider) can classify each token without running the full pipeline.
///
/// # Errors
///
/// See [`lex`].
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    lex(source, 0)
}

/// The mutable lexer state.
struct Lexer<'s> {
    /// Source text being scanned.
    src: &'s str,
    /// Byte offsets of every character, parallel to `chars`.
    bytes: Vec<usize>,
    /// Characters of the source, in order.
    chars: Vec<char>,
    /// Current index into `chars` / `bytes`.
    pos: usize,
    /// Open `( [ {` depth; layout tokens are suppressed above zero.
    bracket: usize,
    /// Indentation-column stack; element zero is the virtual root (0).
    indents: Vec<usize>,
    /// Whether a line boundary at depth zero is pending resolution.
    pending_newline: bool,
    /// Whether any token has been emitted yet (suppresses a leading newline).
    started: bool,
    /// File id for spans.
    file_id: u32,
    /// Emitted tokens.
    out: Vec<Token>,
}

impl<'s> Lexer<'s> {
    fn new(src: &'s str, file_id: u32) -> Self {
        let bytes: Vec<usize> = src.char_indices().map(|(b, _)| b).collect();
        let chars: Vec<char> = src.chars().collect();
        Self {
            src,
            bytes,
            chars,
            pos: 0,
            bracket: 0,
            indents: vec![0],
            pending_newline: false,
            started: false,
            file_id,
            out: Vec::with_capacity(src.len() / 4 + 8),
        }
    }

    fn run(mut self) -> Result<Vec<Token>, LexError> {
        while self.pos < self.chars.len() {
            self.skip_trivia()?;
            if self.pos >= self.chars.len() {
                break;
            }
            // Resolve a pending line boundary before a significant token,
            // unless this is the very first token of the file.
            if self.pending_newline {
                if self.started {
                    self.resolve_newline()?;
                }
                self.pending_newline = false;
            }
            self.started = true;
            self.next_token()?;
        }
        // Close any open layout levels at end of file.
        while self.indents.len() > 1 {
            self.indents.pop();
            self.emit(TokenKind::Dedent, self.src.len(), self.src.len());
        }
        self.emit(TokenKind::Eof, self.src.len(), self.src.len());
        Ok(self.out)
    }

    /// Skips spaces, tabs, comments and (at depth zero) blank lines.
    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') => self.pos += 1,
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(ch) = self.peek() {
                        self.pos += 1;
                        if ch == '\n' {
                            break;
                        }
                    }
                }
                Some('\n') => {
                    self.pos += 1;
                    if self.bracket == 0 {
                        self.pending_newline = true;
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Emits Indent / Dedent / Newline based on `column` of the next token.
    fn resolve_newline(&mut self) -> Result<(), LexError> {
        let col_start = self.line_start();
        let indent = self.current_column();
        let top = *self.indents.last().expect("indent stack non-empty");
        if indent > top {
            self.indents.push(indent);
            self.emit(TokenKind::Newline, col_start, col_start);
            self.emit(TokenKind::Indent, col_start, col_start);
        } else if indent == top {
            self.emit(TokenKind::Newline, col_start, col_start);
        } else {
            while *self.indents.last().expect("indent stack non-empty") > indent {
                self.indents.pop();
                self.emit(TokenKind::Dedent, col_start, col_start);
            }
            if *self.indents.last().expect("indent stack non-empty") != indent {
                return Err(LexError::new(
                    "inconsistent indentation: this line is indented to a level \
                     that matches no enclosing block",
                    Span::new(self.file_id, col_start as u32, col_start as u32 + 1),
                ));
            }
            self.emit(TokenKind::Newline, col_start, col_start);
        }
        Ok(())
    }

    /// Lexes one significant token at `pos`.
    fn next_token(&mut self) -> Result<(), LexError> {
        let start = self.cur_byte();
        let ch = self.peek().expect("caller guarantees a char");
        match ch {
            '$' => self.lex_state_sigil(start),
            ':' => self.take(TokenKind::Colon, 1),
            ',' => self.take(TokenKind::Comma, 1),
            '.' if self.peek_at(1) == Some('.') && self.peek_at(2) == Some('.') => {
                self.take(TokenKind::Ellipsis, 3)
            }
            '.' if self.peek_at(1) == Some('?') => self.take(TokenKind::QuestionDot, 2),
            '.' => self.take(TokenKind::Dot, 1),
            '?' if self.peek_at(1) == Some('.') => self.take(TokenKind::QuestionDot, 2),
            '=' if self.peek_at(1) == Some('>') => self.take(TokenKind::FatArrow, 2),
            '=' if self.peek_at(1) == Some('=') => self.take(TokenKind::EqEq, 2),
            '=' => self.take(TokenKind::Eq, 1),
            '!' if self.peek_at(1) == Some('=') => self.take(TokenKind::NotEq, 2),
            '!' => self.take(TokenKind::Not, 1),
            '<' if self.peek_at(1) == Some('=') => self.take(TokenKind::LtEq, 2),
            '<' => self.take(TokenKind::Lt, 1),
            '>' if self.peek_at(1) == Some('=') => self.take(TokenKind::GtEq, 2),
            '>' => self.take(TokenKind::Gt, 1),
            '-' if self.peek_at(1) == Some('>') => self.take(TokenKind::Arrow, 2),
            '-' if self.is_number(1) => self.lex_number(start),
            '-' => self.take(TokenKind::Minus, 1),
            '@' => self.take(TokenKind::At, 1),
            '(' => self.bracket_tok(TokenKind::LParen),
            ')' => self.bracket_tok(TokenKind::RParen),
            '{' => self.bracket_tok(TokenKind::LBrace),
            '}' => self.bracket_tok(TokenKind::RBrace),
            '[' => self.bracket_tok(TokenKind::LBracket),
            ']' => self.bracket_tok(TokenKind::RBracket),
            '+' => self.take(TokenKind::Plus, 1),
            '*' => self.take(TokenKind::Star, 1),
            '/' => self.take(TokenKind::Slash, 1),
            '%' => self.take(TokenKind::Percent, 1),
            '&' if self.peek_at(1) == Some('&') => self.take(TokenKind::And, 2),
            '|' if self.peek_at(1) == Some('|') => self.take(TokenKind::Or, 2),
            '|' => self.take(TokenKind::Pipe, 1),
            '"' => self.lex_string(start)?,
            '0'..='9' => self.lex_number(start),
            c if c.is_ascii_alphabetic() || c == '_' => self.lex_ident(start),
            other => {
                return Err(LexError::new(
                    format!("unexpected character `{other}`"),
                    Span::new(
                        self.file_id,
                        start as u32,
                        (start + other.len_utf8()) as u32,
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Lexes a `$name` state sigil as a single `Ident` token so the parser can
    /// recognise state declarations uniformly with bare-name references.
    fn lex_state_sigil(&mut self, start: usize) {
        self.pos += 1; // consume '$'
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.out.push(Token {
            kind: TokenKind::Ident,
            start,
            end: self.cur_byte(),
        });
    }

    fn lex_string(&mut self, start: usize) -> Result<(), LexError> {
        self.pos += 1; // opening quote
        let mut depth = 0usize;
        loop {
            match self.peek() {
                None => {
                    return Err(LexError::new(
                        "unterminated string literal",
                        Span::new(self.file_id, start as u32, self.src.len() as u32),
                    ));
                }
                Some('\n') => {
                    return Err(LexError::new(
                        "unterminated string literal",
                        Span::new(self.file_id, start as u32, self.cur_byte() as u32),
                    ));
                }
                Some('\\') => {
                    self.pos += 2;
                }
                Some('{') => {
                    depth += 1;
                    self.pos += 1;
                }
                Some('}') => {
                    depth = depth.saturating_sub(1);
                    self.pos += 1;
                }
                Some('"') if depth == 0 => {
                    self.pos += 1;
                    break;
                }
                Some(_) => self.pos += 1,
            }
        }
        self.out.push(Token {
            kind: TokenKind::Str,
            start,
            end: self.cur_byte(),
        });
        Ok(())
    }

    fn lex_number(&mut self, start: usize) {
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let is_float =
            self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit());
        if is_float {
            self.pos += 1; // '.'
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            self.out.push(Token {
                kind: TokenKind::Float,
                start,
                end: self.cur_byte(),
            });
        } else {
            self.out.push(Token {
                kind: TokenKind::Int,
                start,
                end: self.cur_byte(),
            });
        }
    }

    fn lex_ident(&mut self, start: usize) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = &self.src[start..self.cur_byte()];
        let kind = keyword_kind(text).unwrap_or(TokenKind::Ident);
        self.out.push(Token {
            kind,
            start,
            end: self.cur_byte(),
        });
    }

    /// Whether the char `ahead` positions from `pos` begins a number.
    fn is_number(&self, ahead: usize) -> bool {
        self.peek_at(ahead).is_some_and(|c| c.is_ascii_digit())
    }

    fn take(&mut self, kind: TokenKind, len: usize) {
        let start = self.cur_byte();
        self.pos += len;
        self.out.push(Token {
            kind,
            start,
            end: self.cur_byte(),
        });
    }

    fn bracket_tok(&mut self, kind: TokenKind) {
        let start = self.cur_byte();
        let ch = self.peek().expect("char present");
        if ch == '(' || ch == '[' || ch == '{' {
            self.bracket += 1;
        } else {
            self.bracket = self.bracket.saturating_sub(1);
        }
        self.pos += 1;
        self.out.push(Token {
            kind,
            start,
            end: self.cur_byte(),
        });
    }

    fn emit(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.out.push(Token { kind, start, end });
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn cur_byte(&self) -> usize {
        self.bytes.get(self.pos).copied().unwrap_or(self.src.len())
    }

    /// Column (in characters since the previous newline) at `pos`.
    fn current_column(&self) -> usize {
        let line_start = self.line_start();
        self.src[line_start..self.cur_byte()].chars().count()
    }

    /// Start byte of the current line (used as a zero-width span anchor).
    fn line_start(&self) -> usize {
        self.src[..self.cur_byte()].rfind('\n').map_or(0, |i| i + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_optional_access() {
        let toks = lex("user?.name", 1).expect("lex");
        let kinds: Vec<&str> = toks
            .iter()
            .map(|t| match t.kind {
                TokenKind::Ident => "ident",
                TokenKind::QuestionDot => "?.",
                TokenKind::Dot => ".",
                TokenKind::Eof => "eof",
                _ => "other",
            })
            .collect();
        assert!(
            kinds.contains(&"?."),
            "expected a QuestionDot token in {kinds:?}"
        );
    }
}
