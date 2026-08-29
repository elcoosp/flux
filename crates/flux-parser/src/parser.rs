//! The Flux recursive-descent parser.
//!
//! Consumes the token stream produced by [`crate::lexer`] and builds the
//! surface [`Ast`] (see [`crate::ast`]). The lexer already resolved
//! indentation into `Indent` / `Dedent` / `Newline` tokens at brace
//! depth zero, so this parser delimits component bodies and view children by
//! layout while keeping `{}` for code blocks, handler bodies and record types.
//!
//! The produced AST is shape-compatible with the previously emitted surface
//! tree, so every downstream consumer (type checker, lowerer, codegen, parity)
//! is unaffected by the syntax change.

use flux_syntax::Span;

use crate::ast::{
    Annotation, Arg, Ast, BinOp, Block, BlockItem, CapabilityDecl, ComponentDecl, ConstBinding,
    Decl, Expr, ExprKind, FnDecl, FnName, Ident, ImportDecl, LetPattern, LifecycleKind, MatchArm,
    MatchPattern, MatchPatternKind, MethodSig, Param, Pattern, PropDecl, StateDecl, StrPart,
    TraitDecl, Type, TypeDecl, TypeKindAst, TypeParam, UseDecl, Variant,
};
use crate::error::{Location, ParseError, line_at};
use crate::lexer::{Token, TokenKind, lex};

/// Parses `source` into an [`Ast`].
///
/// `file_id` identifies the file in every produced [`Span`]; `path` is the
/// display path used in diagnostics.
///
/// # Errors
///
/// Returns a [`ParseError`] when `source` is not valid Flux, carrying the
/// what/where/why/how diagnostics required by AGENTS.md §3.7.
pub(crate) fn parse_source(source: &str, file_id: u32, path: &str) -> Result<Ast, ParseError> {
    let tokens = lex(source, file_id).map_err(|err| lex_error_to_parse(source, err, path))?;
    if let Some(err) = check_brace_depth(source, file_id, path) {
        return Err(err);
    }
    let mut parser = Parser {
        tokens: &tokens,
        file_id,
        source,
        path,
        pos: 0,
        block_postfix: true,
    };
    parser.parse_program()
}

/// Maximum brace-nesting depth accepted before the source is rejected with an
/// actionable diagnostic instead of overflowing the call stack.
const MAX_NESTING_DEPTH: usize = 16;

/// Rejects source whose `{` brace nesting exceeds [`MAX_NESTING_DEPTH`], so deeply
/// nested trees fail fast with a hint to extract a component (AGENTS.md §3.7).
fn check_brace_depth(source: &str, file_id: u32, path: &str) -> Option<ParseError> {
    let mut depth = 0usize;
    let mut chars = source.char_indices().peekable();
    while let Some((offset, c)) = chars.next() {
        match c {
            '"' => {
                // Skip string literals so braces inside them don't count.
                let mut interp = 0usize;
                while let Some((_, ch)) = chars.next() {
                    match ch {
                        '\\' => {
                            chars.next();
                        }
                        '{' => interp += 1,
                        '}' => interp = interp.saturating_sub(1),
                        '"' if interp == 0 => break,
                        '\n' => break,
                        _ => {}
                    }
                }
            }
            '{' => {
                depth += 1;
                if depth > MAX_NESTING_DEPTH {
                    return Some(ParseError {
                        message: format!(
                            "block nesting exceeds the maximum depth of {MAX_NESTING_DEPTH}"
                        ),
                        hint: Some(
                            "extract the inner view into its own `compo` \u{2014} deeply nested                              trees are also slower to diff and harder to read"
                                .to_owned(),
                        ),
                        span: Span::new(file_id, offset as u32, offset as u32 + 1),
                        location: Location::from_offset(source, offset),
                        path: path.to_owned(),
                        line_text: line_at(source, offset),
                    });
                }
            }
            '}' => {
                // Closing brace leaves the current block, so true nesting depth
                // drops. Without this the counter only ever grew and any file
                // with more than `MAX_NESTING_DEPTH` braces total (e.g. a real
                // app with >16 components) was wrongly rejected.
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

fn lex_error_to_parse(source: &str, err: crate::lexer::LexError, path: &str) -> ParseError {
    let offset = err.span.start as usize;
    ParseError {
        message: err.message,
        hint: None,
        span: err.span,
        location: Location::from_offset(source, offset),
        path: path.to_owned(),
        line_text: line_at(source, offset),
    }
}

/// The recursive-descent cursor.
struct Parser<'s> {
    /// Token stream (never empty; ends in `Eof`).
    tokens: &'s [Token],
    /// File id for every produced span.
    file_id: u32,
    /// Source text, for span text and diagnostics.
    source: &'s str,
    /// Display path used in diagnostics.
    path: &'s str,
    /// Current index into `tokens`.
    pos: usize,
    /// When `true`, an expression followed by `{` is treated as a `Call` with a
    /// trailing block (e.g. `Column { … }`). Set to `false` while parsing a
    /// `match` scrutinee so the `{` is read as the match body delimiter rather
    /// than a postfix block attached to the scrutinee.
    block_postfix: bool,
}

/// The signature tuple returned by [`Parser::fn_sig`].
type FnSig = (FnName, Vec<TypeParam>, Vec<Param>, Option<Type>, u32);

impl<'s> Parser<'s> {
    // ----- token cursor -----------------------------------------------------

    fn peek(&self) -> Token {
        // Return a sentinel `Eof` token (spanning the end of source) when the
        // cursor is past the end, rather than panicking. Callers use `at(Eof)`
        // to terminate loops and `eat`/`expect` to surface a `ParseError`, so a
        // graceful `Eof` keeps arbitrary/malformed input from crashing the
        // parser (required by `parsing_arbitrary_token_soup_never_panics`).
        match self.tokens.get(self.pos) {
            Some(tok) => *tok,
            None => Token {
                kind: TokenKind::Eof,
                start: self.source.len(),
                end: self.source.len(),
            },
        }
    }

    fn peek_kind(&self) -> TokenKind {
        self.peek().kind
    }

    /// The kind of the token after the current one, if any.
    fn next_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos + 1).map(|t| t.kind)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek_kind() == kind
    }

    /// Advances past a token of `kind`, returning it, or errors if it is absent.
    fn eat(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if self.at(kind) {
            let tok = self.peek();
            self.pos += 1;
            Ok(tok)
        } else {
            Err(self.expect_error(kind))
        }
    }

    /// Advances past a token of `kind`; returns `false` if it is absent.
    fn try_eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Advances past any layout `Newline` trivia. The lexer emits `Newline`
    /// tokens between significant lines; block parsers consume them at their
    /// boundaries so indentation (`Indent`/`Dedent`) drives structure.
    fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) {
            self.pos += 1;
        }
    }

    /// Advances past any layout trivia: `Newline`, `Indent`, and `Dedent`.
    /// Used at the boundaries of brace-delimited bodies (traits, capabilities,
    /// ADT variant lists) where indentation is significant to the lexer but
    /// irrelevant to these manual loops.
    fn skip_layout(&mut self) {
        while matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
        ) {
            self.pos += 1;
        }
    }

    fn span_of(&self, tok: Token) -> Span {
        tok.span(self.file_id)
    }

    fn text_of(&self, tok: Token) -> &'s str {
        &self.source[tok.start..tok.end]
    }

    fn peek_at(&self, ahead: usize) -> Option<TokenKind> {
        self.tokens.get(self.pos + ahead).map(|t| t.kind)
    }

    // ----- diagnostics ------------------------------------------------------

    fn ident_at(&self, tok: Token) -> Ident {
        Ident {
            name: self.text_of(tok).to_owned(),
            span: self.span_of(tok),
        }
    }

    fn error(&self, tok: Token, message: impl Into<String>, hint: Option<String>) -> ParseError {
        let span = self.span_of(tok);
        let offset = span.start as usize;
        ParseError {
            message: message.into(),
            hint,
            span,
            location: Location::from_offset(self.source, offset),
            path: self.path.to_owned(),
            line_text: line_at(self.source, offset),
        }
    }

    fn expect_error(&self, wanted: TokenKind) -> ParseError {
        let tok = self.peek();
        let found = self.text_of(tok).to_owned();
        let where_hint = match wanted {
            TokenKind::Indent => Some("a child line indented past the parent".to_owned()),
            TokenKind::Dedent => Some("a dedented line closing the block".to_owned()),
            TokenKind::LBrace => Some("a `{` opening the block body".to_owned()),
            TokenKind::RParen => Some("a `)` closing the argument list".to_owned()),
            TokenKind::RBrace => Some("add the matching `}` to close this block".to_owned()),
            _ => None,
        };
        self.error(
            tok,
            format!("expected `{}`, found `{}`", kind_name(wanted), found),
            where_hint,
        )
    }

    fn last_end(&self) -> u32 {
        self.tokens[..self.pos]
            .iter()
            .last()
            .map_or(0, |t| t.end as u32)
    }

    // ----- program / declarations ------------------------------------------

    fn parse_program(&mut self) -> Result<Ast, ParseError> {
        let start = self.peek().start;
        let mut decls = Vec::new();
        while !self.at(TokenKind::Eof) {
            self.skip_layout();
            if self.at(TokenKind::Eof) {
                break;
            }
            decls.push(self.decl()?);
        }
        let end = self.peek().end;
        Ok(Ast {
            decls,
            span: Span::new(self.file_id, start as u32, end as u32),
        })
    }

    fn decl(&mut self) -> Result<Decl, ParseError> {
        let tok = self.peek();
        match tok.kind {
            TokenKind::Compo => self.component_decl(),
            TokenKind::Import => self.import_decl(),
            TokenKind::Use => self.use_decl(),
            TokenKind::Fn => self.fn_decl().map(Decl::Fn),
            TokenKind::Type => self.type_decl(),
            TokenKind::Trait => self.trait_decl(),
            TokenKind::Capability => self.capability_decl(),
            TokenKind::At => self.component_decl(),
            TokenKind::Ident if self.is_const_binding() => self.const_binding(),
            _ => Err(self.error(
                tok,
                format!("expected a declaration, found `{}`", self.text_of(tok)),
                Some(
                    "top level accepts `compo`, `import`, `use`, `fn`, `type`, `trait`, `capability`"
                        .to_owned(),
                ),
            )),
        }
    }

    /// `Color.red = …`: a dotted Ident path followed by `=`.
    fn is_const_binding(&self) -> bool {
        if self.peek_kind() != TokenKind::Ident {
            return false;
        }
        let mut p = self.pos + 1;
        if !matches!(self.tokens.get(p).map(|t| t.kind), Some(TokenKind::Dot)) {
            return false;
        }
        p += 1;
        matches!(self.tokens.get(p).map(|t| t.kind), Some(TokenKind::Ident))
            && matches!(self.tokens.get(p + 1).map(|t| t.kind), Some(TokenKind::Eq))
    }

    fn component_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.peek().start;
        let mut annotations = Vec::new();
        while self.at(TokenKind::At) {
            annotations.push(self.annotation()?);
            self.skip_layout();
        }
        self.skip_layout();
        let _comp_kw = self.eat(TokenKind::Compo)?;
        let name = self.ident()?;
        let mut generics = Vec::new();
        if self.at(TokenKind::LBracket) {
            generics = self.generic_params()?;
        }
        let mut props = Vec::new();
        if self.at(TokenKind::LParen) {
            props = self.props_block()?;
        }
        let body = self.indented_block()?;
        let end = body.span.end as usize;
        Ok(Decl::Component(ComponentDecl {
            annotations,
            name,
            generics,
            props,
            body,
            span: Span::new(self.file_id, start as u32, end as u32),
        }))
    }

    fn annotation(&mut self) -> Result<Annotation, ParseError> {
        let start = self.eat(TokenKind::At)?;
        let name = self.ident()?;
        let mut args = Vec::new();
        if self.at(TokenKind::LParen) {
            self.eat(TokenKind::LParen)?;
            while !self.at(TokenKind::RParen) {
                args.push(self.arg()?);
                if !self.try_eat(TokenKind::Comma) {
                    break;
                }
            }
            self.eat(TokenKind::RParen)?;
        }
        let end = self.peek().start.max(name.span.end as usize);
        Ok(Annotation {
            name,
            args,
            span: Span::new(self.file_id, start.start as u32, end as u32),
        })
    }

    fn generic_params(&mut self) -> Result<Vec<TypeParam>, ParseError> {
        self.eat(TokenKind::LBracket)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RBracket) {
            let name_tok = self.ident_tok()?;
            let name = self.ident_at(name_tok);
            let bound = if self.try_eat(TokenKind::Colon) {
                Some(self.ident()?)
            } else {
                None
            };
            params.push(TypeParam {
                name,
                bound,
                span: self.span_of(name_tok),
            });
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        self.eat(TokenKind::RBracket)?;
        Ok(params)
    }

    fn props_block(&mut self) -> Result<Vec<PropDecl>, ParseError> {
        self.eat(TokenKind::LParen)?;
        let mut props = Vec::new();
        while !self.at(TokenKind::RParen) {
            let name = self.ident()?;
            self.eat(TokenKind::Colon)?;
            let ty = self.ty()?;
            let default = if self.try_eat(TokenKind::Eq) {
                Some(self.expr()?)
            } else {
                None
            };
            props.push(PropDecl {
                name: name.clone(),
                ty,
                default,
                span: name.span,
            });
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        self.eat(TokenKind::RParen)?;
        Ok(props)
    }

    fn import_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.eat(TokenKind::Import)?;
        let name = self.ident()?;
        self.eat(TokenKind::Ident)?; // `from`
        let src_tok = self.eat(TokenKind::Str)?;
        let source = unescape(self.text_of(src_tok));
        Ok(Decl::Import(ImportDecl {
            name,
            source,
            span: Span::new(self.file_id, start.start as u32, src_tok.end as u32),
        }))
    }

    fn use_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.eat(TokenKind::Use)?;
        let mut segments = Vec::new();
        segments.push(self.ident()?);
        let mut glob = false;
        while self.try_eat(TokenKind::Colon) {
            self.eat(TokenKind::Colon)?;
            if self.try_eat(TokenKind::Star) {
                glob = true;
                break;
            }
            segments.push(self.ident()?);
        }
        let end = segments.last().map_or(start.end, |i| i.span.end as usize);
        Ok(Decl::Use(UseDecl {
            segments,
            glob,
            span: Span::new(self.file_id, start.start as u32, end as u32),
        }))
    }

    fn fn_sig(&mut self) -> Result<FnSig, ParseError> {
        let start = self.eat(TokenKind::Fn)?;
        let name_tok = self.peek();
        self.pos += 1;
        let name = FnName {
            text: self.text_of(name_tok).to_owned(),
            is_operator: is_operator(self.text_of(name_tok)),
            span: self.span_of(name_tok),
        };
        let mut generics = Vec::new();
        if self.at(TokenKind::LBracket) {
            generics = self.generic_params()?;
        }
        self.eat(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) {
            params.push(self.param()?);
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        self.eat(TokenKind::RParen)?;
        let mut ret = None;
        if self.try_eat(TokenKind::Arrow) {
            ret = Some(self.ty()?);
        }
        Ok((name, generics, params, ret, start.start as u32))
    }

    fn fn_decl(&mut self) -> Result<FnDecl, ParseError> {
        let (name, generics, params, ret, start) = self.fn_sig()?;
        let body = self.braced_block()?;
        Ok(FnDecl {
            name,
            generics,
            params,
            ret,
            body: body.clone(),
            span: Span::new(self.file_id, start, body.span.end),
        })
    }

    fn param(&mut self) -> Result<Param, ParseError> {
        let name = self.ident()?;
        let mut ty = None;
        if self.try_eat(TokenKind::Colon) {
            ty = Some(self.ty()?);
        }
        let default = if self.try_eat(TokenKind::Eq) {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Param {
            name: name.clone(),
            ty,
            default,
            span: name.span,
        })
    }

    fn type_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.eat(TokenKind::Type)?;
        let name = self.ident()?;
        let mut generics = Vec::new();
        if self.at(TokenKind::LBracket) {
            generics = self.generic_params()?;
        }
        self.eat(TokenKind::Eq)?;
        let mut variants = Vec::new();
        let mut is_first = true;
        loop {
            self.skip_layout();
            if is_first {
                self.try_eat(TokenKind::Pipe); // optional leading `|`
                is_first = false;
            } else {
                self.eat(TokenKind::Pipe)?;
            }
            self.skip_layout();
            variants.push(self.variant()?);
            self.skip_layout();
            if !self.at(TokenKind::Pipe) {
                break;
            }
        }
        Ok(Decl::Type(TypeDecl {
            name: name.clone(),
            generics,
            variants,
            span: Span::new(self.file_id, start.start as u32, name.span.end),
        }))
    }

    fn variant(&mut self) -> Result<Variant, ParseError> {
        let name_tok = self.ident_tok()?;
        let name = self.ident_at(name_tok);
        let mut fields = Vec::new();
        if self.at(TokenKind::LParen) {
            self.eat(TokenKind::LParen)?;
            while !self.at(TokenKind::RParen) {
                fields.push(self.ty()?);
                if !self.try_eat(TokenKind::Comma) {
                    break;
                }
            }
            self.eat(TokenKind::RParen)?;
        }
        Ok(Variant {
            name,
            fields,
            span: self.span_of(name_tok),
        })
    }

    fn trait_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.eat(TokenKind::Trait)?;
        let name = self.ident()?;
        let mut generics = Vec::new();
        if self.at(TokenKind::LBracket) {
            generics = self.generic_params()?;
        }
        let (methods, end) = self.method_block()?;
        Ok(Decl::Trait(TraitDecl {
            name,
            generics,
            methods,
            span: Span::new(self.file_id, start.start as u32, end),
        }))
    }

    fn capability_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.eat(TokenKind::Capability)?;
        let name = self.ident()?;
        let (methods, end) = self.method_block()?;
        Ok(Decl::Capability(CapabilityDecl {
            name,
            methods,
            span: Span::new(self.file_id, start.start as u32, end),
        }))
    }

    /// Parses a `{ fn name(args) -> Ty … }` method-signature list. Trait and
    /// capability bodies hold only method signatures, so we read them directly
    /// as [`FnDecl`]s and lift each into a [`MethodSig`] (the AST has no
    /// `ExprKind::Fn` form — method signatures are not expressions).
    fn method_block(&mut self) -> Result<(Vec<MethodSig>, u32), ParseError> {
        let _open = self.eat(TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            self.skip_layout();
            if self.at(TokenKind::RBrace) || self.at(TokenKind::Eof) {
                break;
            }
            methods.push(self.method_sig()?);
            self.try_eat(TokenKind::Newline);
        }
        let close = self.eat(TokenKind::RBrace)?;
        Ok((methods, close.end as u32))
    }

    fn method_sig(&mut self) -> Result<MethodSig, ParseError> {
        let (name, generics, params, ret, start) = self.fn_sig()?;
        Ok(MethodSig {
            name,
            generics,
            params,
            ret,
            span: Span::new(self.file_id, start, self.last_end()),
        })
    }

    fn const_binding(&mut self) -> Result<Decl, ParseError> {
        let mut path = vec![self.ident()?];
        while self.try_eat(TokenKind::Dot) {
            path.push(self.ident()?);
        }
        self.eat(TokenKind::Eq)?;
        let value = self.expr()?;
        let end = value.span.end as usize;
        Ok(Decl::Const(ConstBinding {
            path: path.clone(),
            value,
            span: Span::new(self.file_id, path[0].span.start as u32, end as u32),
        }))
    }

    // ----- blocks -----------------------------------------------------------

    /// Parses an indentation-delimited block: the current line already carries
    /// an `Indent`; items follow until the matching `Dedent`.
    fn indented_block(&mut self) -> Result<Block, ParseError> {
        let start = self.peek().start;
        self.skip_newlines();
        if !self.at(TokenKind::Indent) {
            // `compo A` with no indented children: an empty body.
            return Ok(Block {
                params: Vec::new(),
                items: Vec::new(),
                span: Span::new(self.file_id, start as u32, start as u32),
            });
        }
        let start = self.eat(TokenKind::Indent)?;
        let mut items = Vec::new();
        while !self.at(TokenKind::Dedent) && !self.at(TokenKind::Eof) {
            self.skip_newlines();
            if self.at(TokenKind::Dedent) || self.at(TokenKind::Eof) {
                break;
            }
            items.push(self.block_item()?);
            self.try_eat(TokenKind::Newline);
        }
        let end = self
            .eat(TokenKind::Dedent)
            .map(|t| t.end)
            .unwrap_or(start.end);
        Ok(Block {
            params: Vec::new(),
            items,
            span: Span::new(self.file_id, start.start as u32, end as u32),
        })
    }

    /// Parses a braced code block: `{ ... }`.
    fn braced_block(&mut self) -> Result<Block, ParseError> {
        let start = self.eat(TokenKind::LBrace)?;
        self.skip_newlines();
        let mut params = Vec::new();
        if self.is_block_param_list() {
            params = self.parse_block_params()?;
        }
        let is_prop_block = self.at(TokenKind::Ident) && self.next_kind() == Some(TokenKind::Colon);
        let mut items = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            self.skip_layout();
            if self.at(TokenKind::RBrace) || self.at(TokenKind::Eof) {
                break;
            }
            if is_prop_block {
                let name = self.ident()?;
                self.eat(TokenKind::Colon)?;
                let value = self.expr()?;
                items.push(BlockItem::Prop { name, value });
            } else {
                items.push(self.block_item()?);
            }
            self.try_eat(TokenKind::Newline);
            self.try_eat(TokenKind::Comma); // optional separators
        }
        let end = self.eat(TokenKind::RBrace)?;
        Ok(Block {
            params,
            items,
            span: Span::new(self.file_id, start.start as u32, end.end as u32),
        })
    }

    /// Whether the `{ ... }` at the cursor is an anonymous record literal
    /// (`{ x: 1, y: 2 }`) rather than a code block. True only when there is no
    /// `name =>` block-param header and every top-level entry is `ident: expr`.
    fn looks_like_record_lit(&self) -> bool {
        if self.is_block_param_list() {
            return false;
        }
        // Inspect the first entry: it must be `ident:`.
        let first = self.tokens.get(self.pos + 1);
        match first {
            Some(t) if t.kind == TokenKind::Ident => {
                self.tokens.get(self.pos + 2).map(|t| t.kind) == Some(TokenKind::Colon)
            }
            _ => false,
        }
    }

    /// Parses an anonymous record literal `{ name: expr, ... }` (FLUX-054).
    /// The record carries no type name; the checker treats it as a structural
    /// `TcType::Record`.
    fn record_lit(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::LBrace)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            self.skip_layout();
            if self.at(TokenKind::RBrace) || self.at(TokenKind::Eof) {
                break;
            }
            let name = self.ident()?;
            self.eat(TokenKind::Colon)?;
            let value = self.expr()?;
            fields.push((name, value));
            self.try_eat(TokenKind::Newline);
            self.try_eat(TokenKind::Comma);
        }
        let end = self.eat(TokenKind::RBrace)?;
        let span = Span::new(self.file_id, start.start as u32, end.end as u32);
        // Anonymous records have no type name; use an empty identifier so the
        // checker's `ExprKind::Record` arm produces a structural record.
        let name = Ident {
            name: String::new(),
            span,
        };
        Ok(Expr {
            kind: ExprKind::Record { name, fields },
            span,
        })
    }

    /// Whether the tokens after `{` form a `name (, name)* =>` header.
    fn is_block_param_list(&self) -> bool {
        let mut p = self.pos + 1;
        loop {
            match self.tokens.get(p).map(|t| t.kind) {
                Some(TokenKind::Ident) => p += 1,
                Some(TokenKind::Comma) => p += 1,
                Some(TokenKind::FatArrow) => return true,
                _ => return false,
            }
        }
    }

    fn parse_block_params(&mut self) -> Result<Vec<Pattern>, ParseError> {
        let mut params = Vec::new();
        loop {
            let id = self.ident()?;
            params.push(Pattern::Ident(id));
            if self.try_eat(TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.eat(TokenKind::FatArrow)?;
        Ok(params)
    }

    fn block_item(&mut self) -> Result<BlockItem, ParseError> {
        let tok = self.peek();
        if tok.kind == TokenKind::State || self.is_state_sigil(tok) {
            return self.state_decl();
        }
        let expr = self.expr()?;
        // A view call in the dream syntax reads `Name key: value, key: value`
        // (spaced props, no parentheses) and may own an indented child block.
        // We recognise it here so a trailing `:` prop list is collected into
        // named call arguments rather than a bare `BlockItem::Prop`.
        if matches!(expr.kind, ExprKind::Ident(_)) {
            let mut args = Vec::new();
            // A dream view call reads `Callee prop: value, prop: value` where
            // each property name is an identifier immediately followed by a
            // colon. We collect those into named call arguments.
            while self.at(TokenKind::Ident) && self.next_kind() == Some(TokenKind::Colon) {
                let name = self.ident()?;
                self.eat(TokenKind::Colon)?;
                let value = self.expr()?;
                args.push(Arg::Named { name, value });
                self.try_eat(TokenKind::Comma);
            }
            if !args.is_empty() {
                self.skip_newlines();
                let trailing = if self.at(TokenKind::Indent) {
                    Some(Box::new(self.indented_block()?))
                } else {
                    None
                };
                return Ok(BlockItem::Expr(Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr.clone()),
                        args,
                        trailing,
                    },
                    span: Span::new(self.file_id, expr.span.start, self.last_end()),
                }));
            }
        }
        Ok(BlockItem::Expr(expr))
    }

    fn is_state_sigil(&self, tok: Token) -> bool {
        self.text_of(tok).starts_with('$')
    }

    fn state_decl(&mut self) -> Result<BlockItem, ParseError> {
        let sigil = self.peek();
        let start = sigil.start;
        let name_text = self.text_of(sigil);
        let name = if let Some(stripped) = name_text.strip_prefix('$') {
            self.pos += 1;
            Ident {
                name: stripped.to_owned(),
                span: Span::new(self.file_id, (start + 1) as u32, sigil.end as u32),
            }
        } else {
            self.pos += 1; // consume `state`
            self.ident()?
        };
        let mut ty = None;
        if self.try_eat(TokenKind::Colon) {
            ty = Some(self.ty()?);
        }
        self.eat(TokenKind::Eq)?;
        let init = self.expr()?;
        Ok(BlockItem::State(StateDecl {
            name,
            ty,
            init: init.clone(),
            span: Span::new(self.file_id, start as u32, init.span.end),
        }))
    }

    // ----- expressions ------------------------------------------------------

    fn expr(&mut self) -> Result<Expr, ParseError> {
        self.assign_expr()
    }

    fn assign_expr(&mut self) -> Result<Expr, ParseError> {
        let target = self.or_expr()?;
        if self.at(TokenKind::Eq) {
            self.eat(TokenKind::Eq)?;
            let value = self.expr()?;
            return Ok(Expr {
                kind: ExprKind::Assign {
                    target: Box::new(target.clone()),
                    value: Box::new(value.clone()),
                },
                span: Span::new(self.file_id, target.span.start, value.span.end),
            });
        }
        Ok(target)
    }

    fn or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.and_expr()?;
        while self.at(TokenKind::Or) {
            self.eat(TokenKind::Or)?;
            let rhs = self.and_expr()?;
            lhs = bin(BinOp::Or, lhs, rhs, self.file_id);
        }
        Ok(lhs)
    }

    fn and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.cmp_expr()?;
        while self.at(TokenKind::And) {
            self.eat(TokenKind::And)?;
            let rhs = self.cmp_expr()?;
            lhs = bin(BinOp::And, lhs, rhs, self.file_id);
        }
        Ok(lhs)
    }

    fn cmp_expr(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.add_expr()?;
        let kind = match self.peek_kind() {
            TokenKind::EqEq => Some(BinOp::Eq),
            TokenKind::NotEq => Some(BinOp::Ne),
            TokenKind::Lt => Some(BinOp::Lt),
            TokenKind::Gt => Some(BinOp::Gt),
            TokenKind::LtEq => Some(BinOp::Le),
            TokenKind::GtEq => Some(BinOp::Ge),
            _ => None,
        };
        if let Some(op) = kind {
            self.pos += 1;
            let rhs = self.add_expr()?;
            Ok(bin(op, lhs, rhs, self.file_id))
        } else {
            Ok(lhs)
        }
    }

    fn add_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.mul_expr()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Plus => Some(BinOp::Add),
                TokenKind::Minus => Some(BinOp::Sub),
                _ => None,
            };
            if let Some(op) = op {
                self.pos += 1;
                let rhs = self.mul_expr()?;
                lhs = bin(op, lhs, rhs, self.file_id);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn mul_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.postfix_expr()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Star => Some(BinOp::Mul),
                TokenKind::Slash => Some(BinOp::Div),
                TokenKind::Percent => Some(BinOp::Rem),
                _ => None,
            };
            if let Some(op) = op {
                self.pos += 1;
                let rhs = self.postfix_expr()?;
                lhs = bin(op, lhs, rhs, self.file_id);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn postfix_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.primary()?;
        loop {
            match self.peek_kind() {
                TokenKind::Dot => {
                    self.eat(TokenKind::Dot)?;
                    let field = self.ident()?;
                    expr = Expr {
                        kind: ExprKind::Field {
                            base: Box::new(expr.clone()),
                            field,
                        },
                        span: expr.span,
                    };
                }
                TokenKind::QuestionDot => {
                    self.eat(TokenKind::QuestionDot)?;
                    let field = self.ident()?;
                    expr = Expr {
                        kind: ExprKind::OptField {
                            base: Box::new(expr.clone()),
                            field,
                        },
                        span: expr.span,
                    };
                }
                TokenKind::LParen => {
                    let args = self.call_args()?;
                    let end = self.last_end();
                    expr = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(expr.clone()),
                            args,
                            trailing: None,
                        },
                        span: Span::new(self.file_id, expr.span.start, end),
                    };
                }
                TokenKind::LBrace if self.block_postfix => {
                    let block = self.braced_block()?;
                    // A trailing code block makes `f(args) { … }` / `Column { … }`
                    // agree in shape: a `Call` with the block as `trailing`. If the
                    // left-hand side is *already* a `Call` (e.g. `Button(...) { … }`,
                    // where the `(...)` was consumed in the previous loop iteration),
                    // attach the block as that call's `trailing` rather than wrapping
                    // the call in a second, callee-of-callee `Call`.
                    let end = block.span.end;
                    let span = Span::new(self.file_id, expr.span.start, end);
                    if let ExprKind::Call { trailing, .. } = &mut expr.kind {
                        *trailing = Some(Box::new(block));
                        expr.span = span;
                    } else {
                        expr = Expr {
                            kind: ExprKind::Call {
                                callee: Box::new(expr.clone()),
                                args: Vec::new(),
                                trailing: Some(Box::new(block)),
                            },
                            span,
                        };
                    }
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn call_args(&mut self) -> Result<Vec<Arg>, ParseError> {
        self.eat(TokenKind::LParen)?;
        let mut args = Vec::new();
        while !self.at(TokenKind::RParen) {
            args.push(self.arg()?);
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        self.eat(TokenKind::RParen)?;
        Ok(args)
    }

    fn arg(&mut self) -> Result<Arg, ParseError> {
        let tok = self.peek();
        if tok.kind == TokenKind::Ident && self.peek_at(1) == Some(TokenKind::Colon) {
            let name = self.ident()?;
            self.eat(TokenKind::Colon)?;
            let value = self.expr()?;
            return Ok(Arg::Named { name, value });
        }
        Ok(Arg::Positional(self.expr()?))
    }

    fn list_lit(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::LBracket)?;
        let mut items = Vec::new();
        while !self.at(TokenKind::RBracket) {
            items.push(self.expr()?);
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self.eat(TokenKind::RBracket)?;
        Ok(Expr {
            kind: ExprKind::List(items),
            span: Span::new(self.file_id, start.start as u32, end.end as u32),
        })
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let tok = self.peek();
        match tok.kind {
            TokenKind::Int => {
                self.pos += 1;
                let v = self
                    .text_of(tok)
                    .parse::<i64>()
                    .map_err(|_| self.error(tok, "integer literal out of 64-bit range", None))?;
                Ok(lit(ExprKind::Int(v), self.span_of(tok)))
            }
            TokenKind::Float => {
                self.pos += 1;
                let v = self
                    .text_of(tok)
                    .parse::<f64>()
                    .map_err(|_| self.error(tok, "float literal is not a valid f64", None))?;
                Ok(lit(ExprKind::Float(v), self.span_of(tok)))
            }
            TokenKind::Bool => {
                self.pos += 1;
                Ok(lit(
                    ExprKind::Bool(self.text_of(tok) == "true"),
                    self.span_of(tok),
                ))
            }
            TokenKind::Str => self.string_lit(tok),
            TokenKind::Ident => {
                let text = self.text_of(tok).to_owned();
                self.pos += 1;
                if let Some(stripped) = text.strip_prefix('$') {
                    Ok(lit(
                        ExprKind::Ident(Ident {
                            name: stripped.to_owned(),
                            span: self.span_of(tok),
                        }),
                        self.span_of(tok),
                    ))
                } else if text == "ForEach" {
                    self.for_expr()
                } else if text == "fn" {
                    // A bare `fn` token in value position is a lambda header.
                    self.lambda()
                } else if text == "Null" {
                    // The `Null` literal (FLUX-053 / ADR-0051) — the absent
                    // value inhabiting every `Option[T]`.
                    Ok(lit(ExprKind::Null, self.span_of(tok)))
                } else {
                    Ok(lit(ExprKind::Ident(self.ident_at(tok)), self.span_of(tok)))
                }
            }
            TokenKind::LParen => self.paren_expr(),
            TokenKind::LBracket => self.list_lit(),
            TokenKind::LBrace => {
                // An anonymous record literal `{ x: 1, y: 2 }` parses as a
                // `Record` value (FLUX-054 / ADR-0052). Only switch to the
                // record path when every entry is `ident: expr` and there is no
                // `name =>` block-param header — otherwise keep the existing
                // code-block / lambda behavior.
                if self.looks_like_record_lit() {
                    let record = self.record_lit()?;
                    return Ok(record);
                }
                let block = self.braced_block()?;
                Ok(Expr {
                    kind: ExprKind::Lambda {
                        params: Vec::new(),
                        body: Box::new(block.clone()),
                    },
                    span: block.span,
                })
            }
            TokenKind::Fn => self.lambda(),
            TokenKind::If => self.if_expr(),
            TokenKind::When => self.when_expr(),
            TokenKind::Match => self.match_expr(),
            TokenKind::Ellipsis => {
                self.pos += 1;
                Ok(lit(ExprKind::Elided, self.span_of(tok)))
            }
            TokenKind::Pipe | TokenKind::Or => self.lambda(),
            TokenKind::OnMount => self.unit_lifecycle(LifecycleKind::OnMount),
            TokenKind::OnCleanup => self.unit_lifecycle(LifecycleKind::OnCleanup),
            TokenKind::Effect => self.unit_lifecycle(LifecycleKind::Effect),
            TokenKind::Derived => self.unit_lifecycle(LifecycleKind::Derived),
            TokenKind::Batch => self.unit_lifecycle(LifecycleKind::Batch),
            TokenKind::Untrack => self.unit_lifecycle(LifecycleKind::Untrack),
            TokenKind::Resource => self.resource_expr(),
            TokenKind::Await => self.await_expr(),
            TokenKind::CreateRef => self.create_ref_expr(),
            TokenKind::Provide => self.provide_expr(),
            TokenKind::UseContext => self.use_context_expr(),
            TokenKind::Let => self.let_expr(),
            _ => Err(self.error(
                tok,
                format!("expected an expression, found `{}`", self.text_of(tok)),
                None,
            )),
        }
    }

    fn string_lit(&mut self, tok: Token) -> Result<Expr, ParseError> {
        self.pos += 1;
        let raw = self.text_of(tok);
        let inner_start = tok.start + 1; // just past the opening quote
        let inner = &raw[1..raw.len().saturating_sub(1)];
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut scan = 0usize; // byte offset into `inner`
        while scan < inner.len() {
            let ch = inner[scan..].chars().next().expect("scan < len");
            let ch_len = ch.len_utf8();
            if ch == '\\' {
                let next = inner[scan + ch_len..].chars().next();
                if let Some(next) = next {
                    push_escape(&mut text, next);
                }
                scan += ch_len + next.map_or(0, char::len_utf8);
            } else if ch == '{' {
                if !text.is_empty() {
                    parts.push(StrPart::Text(std::mem::take(&mut text)));
                }
                // Absolute source offset of the `{` we just saw.
                let open = inner_start + scan;
                let (interp, consumed) = self.interp_expr(open)?;
                parts.push(StrPart::Interp(interp));
                scan += consumed;
            } else {
                text.push(ch);
                scan += ch_len;
            }
        }
        if !text.is_empty() {
            parts.push(StrPart::Text(text));
        }
        Ok(lit(ExprKind::Str(parts), self.span_of(tok)))
    }

    /// Parses the expression inside a `{ … }` string interpolation. `open` is
    /// the absolute byte offset of the opening `{` in `self.source`. We locate
    /// the matching `}`, re-lex that slice and parse it as a bare expression,
    /// then shift its spans so they stay relative to the whole source (matching
    /// every other node in the tree).
    fn interp_expr(&mut self, open: usize) -> Result<(Expr, usize), ParseError> {
        let mut depth = 1usize;
        let mut idx = open + 1;
        let bytes = self.source.as_bytes();
        while idx < self.source.len() {
            match bytes[idx] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'"' => {
                    // Skip string contents; interpolations nest braces but a
                    // `}` inside a string belongs to the string, not the interp.
                    idx += 1;
                    while idx < self.source.len() {
                        match bytes[idx] {
                            b'\\' => idx += 1,
                            b'"' => break,
                            _ => {}
                        }
                        idx += 1;
                    }
                }
                _ => {}
            }
            idx += 1;
        }
        if idx >= self.source.len() {
            return Err(ParseError {
                message: "unterminated string interpolation: no closing `}`".to_owned(),
                hint: Some("add the `}` that closes this `{…}` interpolation".to_owned()),
                span: Span::new(self.file_id, open as u32, (open + 1) as u32),
                location: Location::from_offset(self.source, open),
                path: self.path.to_owned(),
                line_text: line_at(self.source, open),
            });
        }
        // The interpolated expression is the slice between the braces.
        let inner = &self.source[open + 1..idx];
        let tokens = lex(inner, self.file_id).map_err(|err| {
            let off = (open + 1) as u32;
            ParseError {
                message: err.message,
                hint: None,
                span: Span::new(self.file_id, err.span.start + off, err.span.end + off),
                location: Location::from_offset(self.source, open + 1 + err.span.start as usize),
                path: self.path.to_owned(),
                line_text: line_at(self.source, open + 1),
            }
        })?;
        let mut sub = Parser {
            tokens: &tokens,
            file_id: self.file_id,
            source: inner,
            path: self.path,
            pos: 0,
            block_postfix: true,
        };
        let expr = sub
            .expr()
            .map_err(|err| shift_parse_error(err, open as u32 + 1, self.source))?;
        // Advance past the closing `}` for the caller.
        self.pos = self
            .tokens
            .iter()
            .position(|t| t.start > idx)
            .unwrap_or(self.tokens.len());
        // `idx` is the index of the closing `}`; report the byte span consumed
        // (including both braces) so the caller can skip past the whole
        // interpolation instead of re-scanning its contents as literal text.
        Ok((shift_spans(expr, open as u32 + 1), idx - open + 1))
    }

    fn paren_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::LParen)?;
        let inner = self.expr()?;
        let end = self.eat(TokenKind::RParen)?;
        Ok(Expr {
            span: Span::new(self.file_id, start.start as u32, end.end as u32),
            kind: inner.kind,
        })
    }

    fn lambda(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().start;
        let mut params = Vec::new();
        match self.peek_kind() {
            TokenKind::Or => {
                self.eat(TokenKind::Or)?;
                if self.at(TokenKind::Or) {
                    self.eat(TokenKind::Or)?; // `||` parameterless
                }
            }
            TokenKind::Pipe => {
                self.eat(TokenKind::Pipe)?;
                loop {
                    params.push(self.param()?);
                    if self.try_eat(TokenKind::Comma) {
                        continue;
                    }
                    break;
                }
                self.eat(TokenKind::Pipe)?;
            }
            TokenKind::Fn => {
                self.eat(TokenKind::Fn)?;
                if self.at(TokenKind::LParen) {
                    self.eat(TokenKind::LParen)?;
                    while !self.at(TokenKind::RParen) {
                        params.push(self.param()?);
                        if !self.try_eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(TokenKind::RParen)?;
                }
            }
            _ => {}
        }
        let body = self.braced_block()?;
        Ok(Expr {
            kind: ExprKind::Lambda {
                params,
                body: Box::new(body.clone()),
            },
            span: Span::new(self.file_id, start as u32, body.span.end),
        })
    }

    fn if_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::If)?;
        let saved = self.block_postfix;
        self.block_postfix = false;
        let cond = self.expr()?;
        self.block_postfix = saved;
        let then_block = self.braced_block()?;
        let mut else_branch = None;
        self.skip_layout();
        if self.try_eat(TokenKind::Else) {
            if self.at(TokenKind::If) {
                else_branch = Some(Box::new(self.if_expr()?));
            } else {
                let blk = self.braced_block()?;
                // `else { … }` lowers to a zero-arg lambda so it shares the
                // `If.else_branch` shape (a trailing-block `Call` would not).
                else_branch = Some(Box::new(Expr {
                    kind: ExprKind::Lambda {
                        params: Vec::new(),
                        body: Box::new(blk.clone()),
                    },
                    span: blk.span,
                }));
            }
        }
        Ok(Expr {
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_block: Box::new(then_block),
                else_branch,
            },
            span: Span::new(self.file_id, start.start as u32, self.last_end()),
        })
    }

    fn when_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::When)?;
        let saved = self.block_postfix;
        self.block_postfix = false;
        let cond = self.expr()?;
        self.block_postfix = saved;
        let then_block = self.braced_block()?;
        let mut otherwise = None;
        self.skip_layout();
        if self.try_eat(TokenKind::Otherwise) {
            otherwise = Some(self.braced_block()?);
        }
        Ok(Expr {
            kind: ExprKind::When {
                cond: Box::new(cond),
                then_block: Box::new(then_block),
                otherwise: otherwise.map(Box::new),
            },
            span: Span::new(self.file_id, start.start as u32, self.last_end()),
        })
    }

    fn match_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::Match)?;
        let saved = self.block_postfix;
        self.block_postfix = false;
        let scrutinee = self.expr()?;
        self.block_postfix = saved;
        self.eat(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(TokenKind::RBrace) {
            arms.push(self.match_arm()?);
            self.try_eat(TokenKind::Newline);
        }
        let end = self.eat(TokenKind::RBrace)?;
        if arms.is_empty() {
            return Err(self.error(
                end,
                "match must have at least one arm",
                Some("add a `pattern => expression` arm, or remove the empty `match`".to_string()),
            ));
        }
        Ok(Expr {
            kind: ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: Span::new(self.file_id, start.start as u32, end.end as u32),
        })
    }

    fn match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let start = self.peek().start;
        let pattern = self.match_pattern()?;
        self.eat(TokenKind::FatArrow)?;
        let body = self.expr()?;
        Ok(MatchArm {
            pattern,
            body: body.clone(),
            span: Span::new(self.file_id, start as u32, body.span.end),
        })
    }

    fn match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        let tok = self.peek();
        let kind = if self.at(TokenKind::Star) {
            self.pos += 1;
            MatchPatternKind::Wildcard
        } else if tok.kind == TokenKind::Int {
            self.pos += 1;
            let value = self
                .text_of(tok)
                .parse::<i64>()
                .map_err(|_| self.error(tok, "integer literal out of 64-bit range", None))?;
            MatchPatternKind::Literal(lit(ExprKind::Int(value), self.span_of(tok)))
        } else if tok.kind == TokenKind::Str {
            self.pos += 1;
            MatchPatternKind::Literal(lit(
                ExprKind::Str(vec![StrPart::Text(unescape(self.text_of(tok)))]),
                self.span_of(tok),
            ))
        } else if tok.kind == TokenKind::Ident {
            let name = self.ident()?;
            let mut fields = Vec::new();
            if self.at(TokenKind::LParen) {
                self.eat(TokenKind::LParen)?;
                while !self.at(TokenKind::RParen) {
                    fields.push(self.pattern()?);
                    if !self.try_eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.eat(TokenKind::RParen)?;
            }
            MatchPatternKind::Variant { name, fields }
        } else {
            return Err(self.error(tok, "expected a match pattern", None));
        };
        Ok(MatchPattern {
            kind,
            span: self.span_of(tok),
        })
    }

    fn pattern(&mut self) -> Result<Pattern, ParseError> {
        let tok = self.peek();
        if self.at(TokenKind::Star) {
            self.pos += 1;
            return Ok(Pattern::Wildcard(self.span_of(tok)));
        }
        let id = self.ident()?;
        Ok(Pattern::Ident(id))
    }

    fn for_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().start as u32;
        self.eat(TokenKind::LParen)?;
        let items = self.expr()?;
        self.try_eat(TokenKind::Comma);
        self.eat(TokenKind::Ident)?; // `key`
        self.eat(TokenKind::Colon)?;
        let key = self.expr()?;
        self.eat(TokenKind::RParen)?;
        let open = self.eat(TokenKind::LBrace)?;
        let binding = self.pattern()?;
        self.eat(TokenKind::FatArrow)?;
        let body_expr = self.expr()?;
        let close = self.eat(TokenKind::RBrace)?;
        let body = Block {
            params: vec![binding],
            items: vec![BlockItem::Expr(body_expr)],
            span: Span::new(self.file_id, open.start as u32, close.end as u32),
        };
        Ok(Expr {
            kind: ExprKind::ForEach {
                items: Box::new(items),
                key: Box::new(key),
                body: Box::new(body),
            },
            span: Span::new(self.file_id, start, close.end as u32),
        })
    }

    fn let_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::Let)?;
        let pattern = self.let_pattern()?;
        let value = if self.try_eat(TokenKind::Eq) {
            Some(Box::new(self.expr()?))
        } else {
            None
        };
        Ok(Expr {
            kind: ExprKind::Let { pattern, value },
            span: Span::new(self.file_id, start.start as u32, self.last_end()),
        })
    }

    fn let_pattern(&mut self) -> Result<LetPattern, ParseError> {
        if self.at(TokenKind::LParen) {
            self.eat(TokenKind::LParen)?;
            let mut parts = Vec::new();
            while !self.at(TokenKind::RParen) {
                parts.push(self.let_pattern()?);
                if !self.try_eat(TokenKind::Comma) {
                    break;
                }
            }
            self.eat(TokenKind::RParen)?;
            return Ok(LetPattern::Tuple(parts));
        }
        if self.at(TokenKind::LBrace) {
            self.eat(TokenKind::LBrace)?;
            let mut idents = Vec::new();
            while !self.at(TokenKind::RBrace) {
                idents.push(self.ident()?);
                if !self.try_eat(TokenKind::Comma) {
                    break;
                }
            }
            self.eat(TokenKind::RBrace)?;
            return Ok(LetPattern::Record(idents));
        }
        Ok(LetPattern::Ident(self.ident()?))
    }

    fn unit_lifecycle(&mut self, kind: LifecycleKind) -> Result<Expr, ParseError> {
        let start = self.eat(match kind {
            LifecycleKind::OnMount => TokenKind::OnMount,
            LifecycleKind::OnCleanup => TokenKind::OnCleanup,
            LifecycleKind::Effect => TokenKind::Effect,
            LifecycleKind::Derived => TokenKind::Derived,
            LifecycleKind::Batch => TokenKind::Batch,
            LifecycleKind::Untrack => TokenKind::Untrack,
        })?;
        let body = self.braced_block()?;
        Ok(Expr {
            kind: ExprKind::Lifecycle {
                kind,
                body: Box::new(body.clone()),
            },
            span: Span::new(self.file_id, start.start as u32, body.span.end),
        })
    }

    fn resource_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::Resource)?;
        self.eat(TokenKind::LParen)?;
        let value = self.expr()?;
        self.eat(TokenKind::RParen)?;
        Ok(Expr {
            kind: ExprKind::Resource(Box::new(value.clone())),
            span: Span::new(self.file_id, start.start as u32, value.span.end),
        })
    }

    fn await_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::Await)?;
        let value = self.expr()?;
        Ok(Expr {
            kind: ExprKind::Await(Box::new(value.clone())),
            span: Span::new(self.file_id, start.start as u32, value.span.end),
        })
    }

    fn create_ref_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::CreateRef)?;
        let mut args = Vec::new();
        if self.at(TokenKind::LBracket) {
            args = self.generic_args()?;
        }
        self.eat(TokenKind::LParen)?;
        self.eat(TokenKind::RParen)?;
        Ok(Expr {
            kind: ExprKind::CreateRef { args },
            span: Span::new(self.file_id, start.start as u32, self.last_end()),
        })
    }

    fn provide_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::Provide)?;
        let context = self.ident()?;
        self.eat(TokenKind::Use)?; // `with`
        let value = self.expr()?;
        Ok(Expr {
            kind: ExprKind::Provide {
                context,
                value: Box::new(value.clone()),
            },
            span: Span::new(self.file_id, start.start as u32, value.span.end),
        })
    }

    fn use_context_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.eat(TokenKind::UseContext)?;
        self.eat(TokenKind::LParen)?;
        let context = self.ident()?;
        self.eat(TokenKind::RParen)?;
        Ok(Expr {
            kind: ExprKind::UseContext(context.clone()),
            span: Span::new(self.file_id, start.start as u32, context.span.end),
        })
    }

    fn generic_args(&mut self) -> Result<Vec<Type>, ParseError> {
        self.eat(TokenKind::LBracket)?;
        let mut args = Vec::new();
        while !self.at(TokenKind::RBracket) {
            args.push(self.ty()?);
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        self.eat(TokenKind::RBracket)?;
        Ok(args)
    }

    // ----- types ------------------------------------------------------------

    fn fn_type(&mut self, start: Span) -> Result<Type, ParseError> {
        self.eat(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) {
            params.push(self.ty()?);
            if !self.try_eat(TokenKind::Comma) {
                break;
            }
        }
        self.eat(TokenKind::RParen)?;
        self.eat(TokenKind::Arrow)?;
        let ret = self.ty()?;
        Ok(Type {
            kind: TypeKindAst::Fn {
                params,
                ret: Box::new(ret),
            },
            span: Span::new(self.file_id, start.start, self.last_end()),
        })
    }

    /// Whether `name` is a built-in scalar primitive (`Int`, `Float`, `Bool`,
    /// `String`, `Unit`) rather than a user-defined named type.
    fn is_primitive_scalar(name: &str) -> bool {
        matches!(name, "Int" | "Float" | "Bool" | "String" | "Unit")
    }

    fn ty(&mut self) -> Result<Type, ParseError> {
        let tok = self.peek();
        match tok.kind {
            TokenKind::Fn => {
                self.pos += 1;
                self.eat(TokenKind::LParen)?;
                let mut params = Vec::new();
                while !self.at(TokenKind::RParen) {
                    params.push(self.ty()?);
                    if !self.try_eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.eat(TokenKind::RParen)?;
                self.eat(TokenKind::Arrow)?;
                let ret = self.ty()?;
                Ok(Type {
                    kind: TypeKindAst::Fn {
                        params,
                        ret: Box::new(ret),
                    },
                    span: self.span_of(tok),
                })
            }
            TokenKind::LBrace => {
                self.eat(TokenKind::LBrace)?;
                let mut fields = Vec::new();
                while !self.at(TokenKind::RBrace) {
                    let name = self.ident()?;
                    self.eat(TokenKind::Colon)?;
                    let ty = self.ty()?;
                    fields.push((name, ty));
                    if !self.try_eat(TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.eat(TokenKind::RBrace)?;
                Ok(Type {
                    kind: TypeKindAst::Record(fields),
                    span: Span::new(self.file_id, tok.start as u32, end.end as u32),
                })
            }
            TokenKind::Ident => {
                let name = self.ident_at(tok);
                self.pos += 1;
                // `Fn(A, B) -> C` is the function-type spelling (uppercase `Fn`).
                if name.name == "Fn" && self.at(TokenKind::LParen) {
                    return self.fn_type(name.span);
                }
                let mut args = Vec::new();
                if self.at(TokenKind::LBracket) {
                    args = self.generic_args()?;
                } else if self.at(TokenKind::LParen) {
                    self.eat(TokenKind::LParen)?;
                    while !self.at(TokenKind::RParen) {
                        args.push(self.ty()?);
                        if !self.try_eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(TokenKind::RParen)?;
                }
                let kind = if Self::is_primitive_scalar(&name.name) {
                    TypeKindAst::Primitive(name.name.clone())
                } else {
                    TypeKindAst::Named {
                        name: name.clone(),
                        args,
                    }
                };
                Ok(Type {
                    kind,
                    span: name.span,
                })
            }
            _ => {
                if tok.kind == TokenKind::Bool {
                    return Err(self.error(tok, "expected a type", None));
                }
                let text = self.text_of(tok).to_owned();
                self.pos += 1;
                Ok(Type {
                    kind: TypeKindAst::Primitive(text),
                    span: self.span_of(tok),
                })
            }
        }
    }

    // ----- identifiers ------------------------------------------------------

    fn ident(&mut self) -> Result<Ident, ParseError> {
        let tok = self.ident_tok()?;
        Ok(self.ident_at(tok))
    }

    fn ident_tok(&mut self) -> Result<Token, ParseError> {
        let tok = self.peek();
        if tok.kind == TokenKind::Ident {
            self.pos += 1;
            Ok(tok)
        } else {
            Err(self.error(
                tok,
                format!("expected an identifier, found `{}`", self.text_of(tok)),
                None,
            ))
        }
    }
}

// ----- free helpers --------------------------------------------------------

fn bin(op: BinOp, lhs: Expr, rhs: Expr, file_id: u32) -> Expr {
    let span = Span::new(file_id, lhs.span.start, rhs.span.end);
    Expr {
        kind: ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        span,
    }
}

fn lit(kind: ExprKind, span: Span) -> Expr {
    Expr { kind, span }
}

fn is_operator(text: &str) -> bool {
    matches!(
        text,
        "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<=" | ">=" | "<" | ">"
    )
}

fn kind_name(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Indent => "indented child",
        TokenKind::Dedent => "dedented line",
        TokenKind::Newline => "newline",
        TokenKind::Compo => "compo",
        TokenKind::Fn => "fn",
        TokenKind::Let => "let",
        TokenKind::If => "if",
        TokenKind::Else => "else",
        TokenKind::When => "when",
        TokenKind::Otherwise => "otherwise",
        TokenKind::Match => "match",
        TokenKind::Use => "use",
        TokenKind::Import => "import",
        TokenKind::Type => "type",
        TokenKind::Trait => "trait",
        TokenKind::Capability => "capability",
        TokenKind::State => "state",
        TokenKind::LBrace => "{",
        TokenKind::RBrace => "}",
        TokenKind::LParen => "(",
        TokenKind::RParen => ")",
        TokenKind::LBracket => "[",
        TokenKind::RBracket => "]",
        TokenKind::Colon => ":",
        TokenKind::Comma => ",",
        TokenKind::Eq => "=",
        TokenKind::FatArrow => "=>",
        TokenKind::Arrow => "->",
        TokenKind::Ident => "identifier",
        _ => "token",
    }
}

fn unescape(raw: &str) -> String {
    let inner = raw.trim_matches('"');
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                push_escape(&mut out, next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn push_escape(out: &mut String, next: char) {
    match next {
        '"' => out.push('"'),
        '{' => out.push('{'),
        '}' => out.push('}'),
        '\\' => out.push('\\'),
        'n' => out.push('\n'),
        't' => out.push('\t'),
        'r' => out.push('\r'),
        other => {
            out.push('\\');
            out.push(other);
        }
    }
}

/// Shifts every byte offset in `expr`'s spans by `delta` so that an expression
/// parsed from a sub-slice (for example a string interpolation) keeps spans
/// relative to the whole source.
fn shift_spans(expr: Expr, delta: u32) -> Expr {
    Expr {
        kind: shift_kind(expr.kind, delta),
        span: shift_span(expr.span, delta),
    }
}

fn shift_span(span: Span, delta: u32) -> Span {
    Span::new(span.file_id, span.start + delta, span.end + delta)
}

fn shift_ident(ident: Ident, delta: u32) -> Ident {
    Ident {
        name: ident.name,
        span: shift_span(ident.span, delta),
    }
}

fn shift_block(block: Block, delta: u32) -> Block {
    Block {
        params: block
            .params
            .into_iter()
            .map(|p| shift_pattern(p, delta))
            .collect(),
        items: block
            .items
            .into_iter()
            .map(|i| shift_block_item(i, delta))
            .collect(),
        span: shift_span(block.span, delta),
    }
}

fn shift_block_item(item: BlockItem, delta: u32) -> BlockItem {
    match item {
        BlockItem::State(decl) => BlockItem::State(StateDecl {
            name: shift_ident(decl.name, delta),
            ty: decl.ty.map(|t| shift_type(t, delta)),
            init: shift_spans(decl.init, delta),
            span: shift_span(decl.span, delta),
        }),
        BlockItem::Prop { name, value } => BlockItem::Prop {
            name: shift_ident(name, delta),
            value: shift_spans(value, delta),
        },
        BlockItem::Expr(expr) => BlockItem::Expr(shift_spans(expr, delta)),
    }
}

fn shift_pattern(pattern: Pattern, delta: u32) -> Pattern {
    match pattern {
        Pattern::Ident(id) => Pattern::Ident(shift_ident(id, delta)),
        Pattern::Wildcard(span) => Pattern::Wildcard(shift_span(span, delta)),
    }
}

fn shift_type(ty: Type, delta: u32) -> Type {
    let kind = match ty.kind {
        TypeKindAst::Named { name, args } => TypeKindAst::Named {
            name: shift_ident(name, delta),
            args: args.into_iter().map(|a| shift_type(a, delta)).collect(),
        },
        TypeKindAst::Record(fields) => TypeKindAst::Record(
            fields
                .into_iter()
                .map(|(n, t)| (shift_ident(n, delta), shift_type(t, delta)))
                .collect(),
        ),
        TypeKindAst::Fn { params, ret } => TypeKindAst::Fn {
            params: params.into_iter().map(|p| shift_type(p, delta)).collect(),
            ret: Box::new(shift_type(*ret, delta)),
        },
        TypeKindAst::Primitive(text) => TypeKindAst::Primitive(text),
    };
    Type {
        kind,
        span: shift_span(ty.span, delta),
    }
}

fn shift_match_arm(arm: MatchArm, delta: u32) -> MatchArm {
    MatchArm {
        pattern: shift_match_pattern(arm.pattern, delta),
        body: shift_spans(arm.body, delta),
        span: shift_span(arm.span, delta),
    }
}

fn shift_match_pattern(pattern: MatchPattern, delta: u32) -> MatchPattern {
    let kind = match pattern.kind {
        MatchPatternKind::Wildcard => MatchPatternKind::Wildcard,
        MatchPatternKind::Literal(expr) => MatchPatternKind::Literal(shift_spans(expr, delta)),
        MatchPatternKind::Variant { name, fields } => MatchPatternKind::Variant {
            name: shift_ident(name, delta),
            fields: fields
                .into_iter()
                .map(|p| shift_pattern(p, delta))
                .collect(),
        },
        MatchPatternKind::Guard { name, cond } => MatchPatternKind::Guard {
            name: shift_ident(name, delta),
            cond: shift_spans(cond, delta),
        },
    };
    MatchPattern {
        kind,
        span: shift_span(pattern.span, delta),
    }
}

fn shift_let_pattern(pattern: LetPattern, delta: u32) -> LetPattern {
    match pattern {
        LetPattern::Ident(id) => LetPattern::Ident(shift_ident(id, delta)),
        LetPattern::Tuple(parts) => LetPattern::Tuple(
            parts
                .into_iter()
                .map(|p| shift_let_pattern(p, delta))
                .collect(),
        ),
        LetPattern::Record(idents) => {
            LetPattern::Record(idents.into_iter().map(|i| shift_ident(i, delta)).collect())
        }
    }
}

fn shift_kind(kind: ExprKind, delta: u32) -> ExprKind {
    match kind {
        ExprKind::Int(v) => ExprKind::Int(v),
        ExprKind::Float(v) => ExprKind::Float(v),
        ExprKind::Bool(v) => ExprKind::Bool(v),
        ExprKind::Null => ExprKind::Null,
        ExprKind::Str(parts) => ExprKind::Str(
            parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Text(t) => StrPart::Text(t),
                    StrPart::Interp(e) => StrPart::Interp(shift_spans(e, delta)),
                })
                .collect(),
        ),
        ExprKind::List(items) => {
            ExprKind::List(items.into_iter().map(|e| shift_spans(e, delta)).collect())
        }
        ExprKind::Ident(id) => ExprKind::Ident(shift_ident(id, delta)),
        ExprKind::Elided => ExprKind::Elided,
        ExprKind::Record { name, fields } => ExprKind::Record {
            name: shift_ident(name, delta),
            fields: fields
                .into_iter()
                .map(|(n, e)| (shift_ident(n, delta), shift_spans(e, delta)))
                .collect(),
        },
        ExprKind::Binary { op, lhs, rhs } => ExprKind::Binary {
            op,
            lhs: Box::new(shift_spans(*lhs, delta)),
            rhs: Box::new(shift_spans(*rhs, delta)),
        },
        ExprKind::Field { base, field } => ExprKind::Field {
            base: Box::new(shift_spans(*base, delta)),
            field: shift_ident(field, delta),
        },
        ExprKind::OptField { base, field } => ExprKind::OptField {
            base: Box::new(shift_spans(*base, delta)),
            field: shift_ident(field, delta),
        },
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => ExprKind::Call {
            callee: Box::new(shift_spans(*callee, delta)),
            args: args
                .into_iter()
                .map(|a| match a {
                    Arg::Positional(e) => Arg::Positional(shift_spans(e, delta)),
                    Arg::Named { name, value } => Arg::Named {
                        name: shift_ident(name, delta),
                        value: shift_spans(value, delta),
                    },
                })
                .collect(),
            trailing: trailing.map(|b| Box::new(shift_block(*b, delta))),
        },
        ExprKind::Let { pattern, value } => ExprKind::Let {
            pattern: shift_let_pattern(pattern, delta),
            value: value.map(|e| Box::new(shift_spans(*e, delta))),
        },
        ExprKind::Assign { target, value } => ExprKind::Assign {
            target: Box::new(shift_spans(*target, delta)),
            value: Box::new(shift_spans(*value, delta)),
        },
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => ExprKind::If {
            cond: Box::new(shift_spans(*cond, delta)),
            then_block: Box::new(shift_block(*then_block, delta)),
            else_branch: else_branch.map(|e| Box::new(shift_spans(*e, delta))),
        },
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => ExprKind::When {
            cond: Box::new(shift_spans(*cond, delta)),
            then_block: Box::new(shift_block(*then_block, delta)),
            otherwise: otherwise.map(|b| Box::new(shift_block(*b, delta))),
        },
        ExprKind::Match { scrutinee, arms } => ExprKind::Match {
            scrutinee: Box::new(shift_spans(*scrutinee, delta)),
            arms: arms
                .into_iter()
                .map(|a| shift_match_arm(a, delta))
                .collect(),
        },
        ExprKind::ForEach { items, key, body } => ExprKind::ForEach {
            items: Box::new(shift_spans(*items, delta)),
            key: Box::new(shift_spans(*key, delta)),
            body: Box::new(shift_block(*body, delta)),
        },
        ExprKind::Provide { context, value } => ExprKind::Provide {
            context: shift_ident(context, delta),
            value: Box::new(shift_spans(*value, delta)),
        },
        ExprKind::UseContext(id) => ExprKind::UseContext(shift_ident(id, delta)),
        ExprKind::Lambda { params, body } => ExprKind::Lambda {
            params: params.into_iter().map(|p| shift_param(p, delta)).collect(),
            body: Box::new(shift_block(*body, delta)),
        },
        ExprKind::Lifecycle { kind, body } => ExprKind::Lifecycle {
            kind,
            body: Box::new(shift_block(*body, delta)),
        },
        ExprKind::Resource(e) => ExprKind::Resource(Box::new(shift_spans(*e, delta))),
        ExprKind::Await(e) => ExprKind::Await(Box::new(shift_spans(*e, delta))),
        ExprKind::CreateRef { args } => ExprKind::CreateRef {
            args: args.into_iter().map(|a| shift_type(a, delta)).collect(),
        },
    }
}

fn shift_param(param: Param, delta: u32) -> Param {
    Param {
        name: shift_ident(param.name, delta),
        ty: param.ty.map(|t| shift_type(t, delta)),
        default: param.default.map(|e| shift_spans(e, delta)),
        span: shift_span(param.span, delta),
    }
}

/// Shifts a sub-parse error's spans to be relative to the whole source.
fn shift_parse_error(mut err: ParseError, delta: u32, source: &str) -> ParseError {
    err.span = shift_span(err.span, delta);
    err.location = Location::from_offset(source, err.span.start as usize);
    err.line_text = line_at(source, err.span.start as usize);
    err
}
