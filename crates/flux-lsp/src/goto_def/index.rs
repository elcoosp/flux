use flux_parser::ast::{
    Ast, Block, BlockItem, ComponentDecl, Decl, Expr, ExprKind, FnDecl, LetPattern,
    MatchPatternKind, Pattern,
};
use flux_syntax::Span;

/// A single binding site in the source.
#[derive(Clone, Debug)]
struct Def {
    /// The bound name.
    name: String,
    /// Span of the *definition* (the name as written where it is introduced).
    def_span: Span,
    /// Span of the scope in which this binding is visible. Resolution only
    /// returns this definition for cursors that fall inside `scope`.
    scope: Span,
}

/// An index of every binding site in a file, used to resolve a cursor to the
/// declaration it refers to.
pub(crate) struct DefIndex {
    defs: Vec<Def>,
}

impl DefIndex {
    /// Builds the index by walking the whole AST.
    #[must_use]
    pub(crate) fn build(ast: &Ast) -> Self {
        let mut defs: Vec<Def> = Vec::new();
        for decl in &ast.decls {
            index_decl(decl, &mut defs);
        }
        Self { defs }
    }

    /// Resolves `cursor` (a byte offset) to the declaration span of the symbol
    /// under it, honouring lexical shadowing (the tightest enclosing in-scope
    /// binding wins). Returns `None` when no binding matches.
    #[must_use]
    pub(crate) fn resolve(&self, text: &str, cursor: u32) -> Option<Span> {
        let word = word_at(text, cursor)?;
        let mut found: Option<Span> = None;
        let mut best_scope: Option<Span> = None;
        for def in &self.defs {
            if def.name == word && def.scope.contains(cursor) {
                // Prefer the tightest enclosing scope.
                if best_scope.is_none_or(|b| def.scope.len() < b.len()) {
                    found = Some(def.def_span);
                    best_scope = Some(def.scope);
                }
            }
        }
        found
    }
}

/// Records a binding with its definition span and visible scope.
fn push(defs: &mut Vec<Def>, name: &str, def_span: Span, scope: Span) {
    defs.push(Def {
        name: name.to_owned(),
        def_span,
        scope,
    });
}

fn index_decl(decl: &Decl, defs: &mut Vec<Def>) {
    match decl {
        Decl::Component(c) => index_component(c, defs),
        Decl::Fn(f) => index_fn(f, defs),
        // Import/Use/Type/Trait/Capability/Const introduce names at module
        // scope, but the MLP editor's go-to-definition targets component and
        // value bindings; the rest are out of scope for now.
        _ => {}
    }
}

fn index_component(c: &ComponentDecl, defs: &mut Vec<Def>) {
    // The component name is visible from the opening brace of its body onward,
    // so a bare `Counter()` occurrence below resolves to the declaration.
    push(defs, &c.name.name, c.name.span, c.body.span);
    for prop in &c.props {
        push(defs, &prop.name.name, prop.name.span, c.body.span);
    }
    index_block(&c.body, defs);
}

fn index_fn(f: &FnDecl, defs: &mut Vec<Def>) {
    let body = &f.body;
    for param in &f.params {
        push(defs, &param.name.name, param.name.span, body.span);
    }
    index_block(body, defs);
}

fn index_block(block: &Block, defs: &mut Vec<Def>) {
    for param in &block.params {
        index_pattern(param, defs, block.span);
    }
    for item in &block.items {
        match item {
            BlockItem::State(s) => push(defs, &s.name.name, s.name.span, block.span),
            BlockItem::Prop { name, .. } => push(defs, &name.name, name.span, block.span),
            BlockItem::Expr(e) => index_expr(e, defs),
            _ => {}
        }
    }
}

fn index_expr(expr: &Expr, defs: &mut Vec<Def>) {
    match &expr.kind {
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            index_expr(callee, defs);
            for arg in args {
                index_expr(arg.value(), defs);
            }
            if let Some(block) = trailing {
                index_block(block, defs);
            }
        }
        ExprKind::Let { pattern, value } => {
            index_let_pattern(pattern, defs, expr.span);
            if let Some(v) = value {
                index_expr(v, defs);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            index_expr(scrutinee, defs);
            for arm in arms {
                index_match_pattern(&arm.pattern.kind, defs, arm.pattern.span);
                index_expr(&arm.body, defs);
            }
        }
        ExprKind::Lambda { params, body } => {
            for param in params {
                push(defs, &param.name.name, param.name.span, body.span);
            }
            index_block(body, defs);
        }
        ExprKind::ForEach { items, key, body } => {
            index_expr(items, defs);
            index_expr(key, defs);
            index_block(body, defs);
        }
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            index_expr(cond, defs);
            index_block(then_block, defs);
            if let Some(eb) = else_branch {
                index_expr(eb, defs);
            }
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            index_expr(cond, defs);
            index_block(then_block, defs);
            if let Some(ob) = otherwise {
                index_block(ob, defs);
            }
        }
        ExprKind::Record { fields, .. } => {
            for (_, value) in fields {
                index_expr(value, defs);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            index_expr(lhs, defs);
            index_expr(rhs, defs);
        }
        ExprKind::Field { base, .. } | ExprKind::OptField { base, .. } => index_expr(base, defs),
        ExprKind::Assign { target, value } => {
            index_expr(target, defs);
            index_expr(value, defs);
        }
        ExprKind::List(items) => {
            for it in items {
                index_expr(it, defs);
            }
        }
        ExprKind::Str(parts) => {
            for p in parts {
                if let flux_parser::ast::StrPart::Interp(e) = p {
                    index_expr(e, defs);
                }
            }
        }
        ExprKind::Provide { value, .. } => index_expr(value, defs),
        ExprKind::Resource(e) => index_expr(e, defs),
        ExprKind::Await(e) => index_expr(e, defs),
        ExprKind::Lifecycle { body, .. } => index_block(body, defs),
        _ => {}
    }
}

fn index_let_pattern(pattern: &LetPattern, defs: &mut Vec<Def>, scope: Span) {
    match pattern {
        LetPattern::Ident(id) => push(defs, &id.name, id.span, scope),
        LetPattern::Tuple(pats) => {
            for p in pats {
                index_let_pattern(p, defs, scope);
            }
        }
        LetPattern::Record(ids) => {
            for id in ids {
                push(defs, &id.name, id.span, scope);
            }
        }
        _ => {}
    }
}

fn index_pattern(pattern: &Pattern, defs: &mut Vec<Def>, scope: Span) {
    match pattern {
        Pattern::Ident(id) => push(defs, &id.name, id.span, scope),
        Pattern::Wildcard(_) => {}
        _ => {}
    }
}

fn index_match_pattern(kind: &MatchPatternKind, defs: &mut Vec<Def>, scope: Span) {
    match kind {
        MatchPatternKind::Variant { name, fields } => {
            push(defs, &name.name, name.span, scope);
            for f in fields {
                index_pattern(f, defs, scope);
            }
        }
        MatchPatternKind::Guard { name, cond } => {
            push(defs, &name.name, name.span, scope);
            index_expr(cond, defs);
        }
        MatchPatternKind::Literal(e) => index_expr(e, defs),
        MatchPatternKind::Wildcard => {}
        _ => {}
    }
}

/// Returns the identifier word straddling `cursor` (or `None` when the cursor
/// is not on an identifier character).
fn word_at(text: &str, cursor: u32) -> Option<String> {
    let bytes = text.as_bytes();
    let c = usize::try_from(cursor).ok()?;
    if c >= bytes.len() || !is_ident_byte(bytes[c]) {
        return None;
    }
    let mut start = c;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = c + 1;
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    Some(text[start..end].to_owned())
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
