//! Pretty-printing for [`crate::ast::Expr`] and [`crate::ast::Block`].
//!
//! This is the heart of `flux fmt`: it re-emits expressions from the parsed AST
//! in the canonical indentation-delimited "dream" surface (ADR-0029). Because
//! the parser special-cases two block shapes — the code block (child views) and
//! the prop block (`Image(url) { width: size }`) — the printer mirrors that
//! distinction exactly so that `parse(print(parse(src)))` is stable.
//!
//! Operator precedence is preserved with explicit parentheses where the
//! surrounding context would otherwise re-associate a sub-expression.

use std::fmt::Write;

use crate::ast::Arg;
use crate::ast::BinOp;
use crate::ast::Block;
use crate::ast::BlockItem;
use crate::ast::Expr;
use crate::ast::ExprKind;
use crate::ast::FnName;
use crate::ast::LetPattern;
use crate::ast::LifecycleKind;
use crate::ast::MatchPattern;
use crate::ast::MatchPatternKind;
use crate::ast::Param;
use crate::ast::Pattern;
use crate::ast::StrPart;
use crate::fmt::indent_str;

/// The minimum binary-operator precedence. Used as the base when no parent
/// operator constrains a sub-expression.
const PREC_BASE: u8 = 1;

/// Returns the textual spelling of a binary operator.
fn binop_spelling(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

/// Returns the binding precedence of a binary operator (higher binds tighter).
fn binop_precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 5,
    }
}

/// Whether `ty` is a code block (child views / statements) rather than a prop
/// block (`Image(url) { width: size }`). A block with no items is treated as a
/// code block to keep the empty-body shape stable.
fn is_code_block(block: &Block) -> bool {
    !block
        .items
        .iter()
        .all(|item| matches!(item, BlockItem::Prop { .. }))
}

/// Appends a single block item at zero indentation (for inline braced bodies).
fn write_block_item(out: &mut String, item: &BlockItem, indent: usize) {
    match item {
        BlockItem::State(state) => {
            out.push_str("state ");
            out.push_str(&state.name.name);
            if let Some(ty) = &state.ty {
                out.push_str(": ");
                crate::fmt::ty::write_type(out, ty);
            }
            out.push_str(" = ");
            write_expr(out, &state.init, indent);
        }
        BlockItem::Derived(derived) => {
            out.push_str("derived ");
            out.push_str(&derived.name.name);
            if let Some(ty) = &derived.ty {
                out.push_str(": ");
                crate::fmt::ty::write_type(out, ty);
            }
            out.push_str(" = ");
            write_expr(out, &derived.init, indent);
        }
        BlockItem::Prop { name, value } => {
            out.push_str(&name.name);
            out.push_str(": ");
            write_expr(out, value, indent);
        }
        BlockItem::Expr(expr) => write_expr(out, expr, indent),
    }
}

/// Appends a component/`fn` body as an indentation-delimited block: the header
/// line is at `indent`, each child at `indent + 1`, no braces. This is the
/// canonical shape the parser's `indented_block` reads for `compo`/`fn` bodies
/// (as distinct from braced code/prop blocks).
pub(crate) fn write_indented_block(out: &mut String, block: &Block, indent: usize) {
    let child = indent + 1;
    out.push('\n');
    for item in &block.items {
        out.push_str(&indent_str(child));
        write_block_item(out, item, child);
        out.push('\n');
    }
}

/// Appends `block` at `indent` levels, choosing the canonical shape:
/// * prop block — `name: value` lines, for a view-call prop block;
/// * code block — indented child lines (statements / views);
/// * `forEach` body — `binding =>` then an indented single body line.
pub(crate) fn write_block(out: &mut String, block: &Block, indent: usize, is_for_each: bool) {
    if is_for_each {
        out.push_str(" { ");
        debug_assert_eq!(block.params.len(), 1);
        if let Some(param) = block.params.first() {
            write_pattern(out, param);
        }
        out.push_str(" =>\n");
        let child = indent + 1;
        if let Some(BlockItem::Expr(body)) = block.items.first() {
            out.push_str(&indent_str(child));
            write_expr(out, body, child);
        }
        out.push('\n');
        out.push_str(&indent_str(indent));
        out.push('}');
        return;
    }

    if is_code_block(block) {
        out.push_str(" {\n");
        let child = indent + 1;
        for item in &block.items {
            out.push_str(&indent_str(child));
            write_block_item(out, item, child);
            out.push('\n');
        }
        out.push_str(&indent_str(indent));
        out.push('}');
    } else {
        out.push_str(" {\n");
        let child = indent + 1;
        for item in &block.items {
            if let BlockItem::Prop { name, value } = item {
                out.push_str(&indent_str(child));
                out.push_str(&name.name);
                out.push_str(": ");
                write_expr(out, value, child);
                out.push('\n');
            }
        }
        out.push_str(&indent_str(indent));
        out.push('}');
    }
}

/// Appends a lambda parameter list (`a, b`), with no surrounding delimiters.
fn write_lambda_params(out: &mut String, params: &[Param]) {
    for (index, param) in params.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&param.name.name);
        if let Some(ty) = &param.ty {
            out.push_str(": ");
            crate::fmt::ty::write_type(out, ty);
        }
    }
}

/// Appends a closing `)`-delimited argument list (call or annotation args).
fn write_args(out: &mut String, args: &[Arg], indent: usize) {
    out.push('(');
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        match arg {
            Arg::Positional(expr) => write_expr(out, expr, indent),
            Arg::Named { name, value } => {
                out.push_str(&name.name);
                out.push_str(": ");
                write_expr(out, value, indent);
            }
        }
    }
    out.push(')');
}

/// Appends a binding pattern (`item`, `_`, `a, b`, `{ a, b }`).
fn write_pattern(out: &mut String, pattern: &Pattern) {
    match pattern {
        Pattern::Ident(ident) => out.push_str(&ident.name),
        Pattern::Wildcard(_) => out.push('_'),
    }
}

/// Appends a `let`-binding pattern (`x`, `(a, b)`, `{ a, b }`).
fn write_let_pattern(out: &mut String, pattern: &LetPattern) {
    match pattern {
        LetPattern::Ident(ident) => out.push_str(&ident.name),
        LetPattern::Tuple(parts) => {
            out.push('(');
            for (index, part) in parts.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_let_pattern(out, part);
            }
            out.push(')');
        }
        LetPattern::Record(idents) => {
            out.push('{');
            for (index, ident) in idents.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&ident.name);
            }
            out.push('}');
        }
    }
}

/// Appends a `match` arm pattern (`_`, `Circle(r)`, `0`, `n if n > 0`).
fn write_match_pattern(out: &mut String, pattern: &MatchPattern) {
    match &pattern.kind {
        MatchPatternKind::Wildcard => out.push('_'),
        MatchPatternKind::Literal(expr) => write_expr(out, expr, 0),
        MatchPatternKind::Variant { name, fields } => {
            out.push_str(&name.name);
            if !fields.is_empty() {
                out.push('(');
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    write_pattern(out, field);
                }
                out.push(')');
            }
        }
        MatchPatternKind::Guard { name, cond } => {
            out.push_str(&name.name);
            out.push_str(" if ");
            write_expr(out, cond, 0);
        }
    }
}

/// Appends a function/method name, quoting operator names as written.
pub(crate) fn write_fn_name(out: &mut String, name: &FnName) {
    out.push_str(&name.text);
}

/// Appends a string literal, re-escaping its pieces canonically.
fn write_string(out: &mut String, parts: &[StrPart]) {
    out.push('"');
    for part in parts {
        match part {
            StrPart::Text(text) => {
                for ch in text.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        '\r' => out.push_str("\\r"),
                        other => out.push(other),
                    }
                }
            }
            StrPart::Interp(expr) => {
                out.push('{');
                write_expr(out, expr, 0);
                out.push('}');
            }
        }
    }
    out.push('"');
}

/// Appends a float literal canonically: always with a decimal point, normalized
/// via `format!("{val:.6?}")` is avoided — we re-render with sufficient
/// precision and trim trailing zeros so `0.5` and `0.500000` both print `0.5`.
fn write_float(out: &mut String, val: f64) {
    if val.is_nan() {
        out.push_str("NaN");
        return;
    }
    if val.is_infinite() {
        if val < 0.0 {
            out.push_str("-inf");
        } else {
            out.push_str("inf");
        }
        return;
    }
    // Render with full round-trip precision, then ensure a decimal point so the
    // lexer classifies it as a float and not an int.
    let mut rendered = format!("{val:.15}");
    // Trim trailing zeros after the decimal point.
    if let Some(dot) = rendered.find('.') {
        let trimmed = rendered.trim_end_matches('0');
        if trimmed.ends_with('.') {
            rendered.truncate(dot + 2); // keep one trailing zero: `1.0`
        } else {
            rendered = trimmed.to_owned();
        }
    }
    if !rendered.contains('.') {
        // Whole number: the lexer needs a decimal to read a float.
        let _ = write!(out, "{val:.1}");
        return;
    }
    out.push_str(&rendered);
}

/// Appends `expr` at `indent` levels, wrapping it in parentheses whenever its
/// precedence is lower than `parent_prec` so the parse round-trips unchanged.
pub(crate) fn write_expr(out: &mut String, expr: &Expr, indent: usize) {
    write_expr_prec(out, expr, indent, PREC_BASE)
}

/// Like [`write_expr`] but honours binary-operator precedence.
fn write_expr_prec(out: &mut String, expr: &Expr, indent: usize, parent_prec: u8) {
    match &expr.kind {
        ExprKind::Int(value) => {
            let _ = write!(out, "{value}");
        }
        ExprKind::Float(value) => write_float(out, *value),
        ExprKind::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        ExprKind::Null => out.push_str("Null"),
        ExprKind::Elided => out.push_str("..."),
        ExprKind::Str(parts) => write_string(out, parts),
        ExprKind::Ident(ident) if ident.name.is_empty() => {
            // An empty-name ident only appears for anonymous record literals;
            // never print it standalone.
        }
        ExprKind::Ident(ident) => out.push_str(&ident.name),
        ExprKind::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_expr(out, item, indent);
            }
            out.push(']');
        }
        ExprKind::Record { name, fields } => {
            if !name.name.is_empty() {
                out.push_str(&name.name);
                out.push(' ');
            }
            out.push_str("{ ");
            for (index, (field, value)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&field.name);
                out.push_str(": ");
                write_expr(out, value, indent);
            }
            out.push_str(" }");
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let prec = binop_precedence(*op);
            let needs_parens = parent_prec > prec;
            if needs_parens {
                out.push('(');
            }
            write_expr_prec(out, lhs, indent, prec);
            out.push(' ');
            out.push_str(binop_spelling(*op));
            out.push(' ');
            write_expr_prec(out, rhs, indent, prec);
            if needs_parens {
                out.push(')');
            }
        }
        ExprKind::Field { base, field } => {
            write_expr(out, base, indent);
            out.push('.');
            out.push_str(&field.name);
        }
        ExprKind::OptField { base, field } => {
            write_expr(out, base, indent);
            out.push_str("?.");
            out.push_str(&field.name);
        }
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            write_expr(out, callee, indent);
            match (args.is_empty(), trailing.is_some()) {
                // A `Call` node with no arguments and no trailing block must keep
                // its parentheses (`Home()`) so it round-trips as a `Call` rather
                // than collapsing to a bare `Ident` (which the parser reads as a
                // different node kind).
                (true, false) => out.push_str("()"),
                // A view call with children but no arguments: `Column { … }`.
                (true, true) => {
                    if let Some(block) = trailing {
                        write_block(out, block, indent, false);
                    }
                }
                // A call with arguments: `Button(text: "x")`, optionally with a
                // trailing block.
                (false, _) => {
                    write_args(out, args, indent);
                    if let Some(block) = trailing {
                        write_block(out, block, indent, false);
                    }
                }
            }
        }
        ExprKind::Let { pattern, value } => {
            out.push_str("let ");
            write_let_pattern(out, pattern);
            if let Some(value) = value {
                out.push_str(" = ");
                write_expr(out, value, indent);
            }
        }
        ExprKind::Assign { target, value } => {
            write_expr(out, target, indent);
            out.push_str(" = ");
            write_expr(out, value, indent);
        }
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            out.push_str("if ");
            write_expr(out, cond, indent);
            write_block(out, then_block, indent, false);
            if let Some(else_branch) = else_branch {
                out.push(' ');
                out.push_str("else ");
                match &else_branch.kind {
                    ExprKind::If { .. } => write_expr(out, else_branch, indent),
                    ExprKind::Lambda { body, .. } => write_block(out, body, indent, false),
                    _ => write_expr(out, else_branch, indent),
                }
            }
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            out.push_str("when ");
            write_expr(out, cond, indent);
            write_block(out, then_block, indent, false);
            if let Some(otherwise) = otherwise {
                out.push(' ');
                out.push_str("otherwise");
                write_block(out, otherwise, indent, false);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            out.push_str("match ");
            write_expr(out, scrutinee, indent);
            out.push_str(" {\n");
            let child = indent + 1;
            for arm in arms {
                out.push_str(&indent_str(child));
                write_match_pattern(out, &arm.pattern);
                out.push_str(" => ");
                write_expr(out, &arm.body, child);
                out.push('\n');
            }
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        ExprKind::ForEach { items, key, body } => {
            out.push_str("ForEach(");
            write_expr(out, items, 0);
            out.push_str(", key: ");
            write_expr(out, key, 0);
            out.push(')');
            write_block(out, body, indent, true);
        }
        ExprKind::Provide { context, value } => {
            out.push_str("provide ");
            out.push_str(&context.name);
            out.push_str(" with ");
            write_expr(out, value, indent);
        }
        ExprKind::UseContext(ident) => {
            out.push_str("useContext(");
            out.push_str(&ident.name);
            out.push(')');
        }
        ExprKind::Lambda { params, body } => {
            if params.is_empty() {
                out.push_str("||");
            } else {
                out.push('|');
                write_lambda_params(out, params);
                out.push('|');
            }
            write_block(out, body, indent, false);
        }
        ExprKind::Lifecycle { kind, body } => {
            out.push_str(lifecycle_name(*kind));
            write_block(out, body, indent, false);
        }
        ExprKind::Resource(expr) => {
            out.push_str("resource(");
            write_expr(out, expr, 0);
            out.push(')');
        }
        ExprKind::Await(expr) => {
            out.push_str("await ");
            write_expr(out, expr, indent);
        }
        ExprKind::CreateRef { args } => {
            out.push_str("createRef");
            if !args.is_empty() {
                out.push('[');
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    crate::fmt::ty::write_type(out, arg);
                }
                out.push(']');
            }
            out.push_str("()");
        }
    }
}

/// Returns the keyword spelling of a lifecycle form.
fn lifecycle_name(kind: LifecycleKind) -> &'static str {
    match kind {
        LifecycleKind::OnMount => "onMount",
        LifecycleKind::OnCleanup => "onCleanup",
        LifecycleKind::Effect => "effect",
        LifecycleKind::Derived => "derived",
        LifecycleKind::Batch => "batch",
        LifecycleKind::Untrack => "untrack",
    }
}
