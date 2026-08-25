//! Kotlin view-tree parsers: recovers individual [`ViewNode`] subtrees from
//! emitted Compose source (after the component/`remember` wrapper has been stripped).
//!
//! These helpers are invoked by [`super::parse_body`] while walking a `@Composable
//! fun`'s body block. They are private to the Kotlin recognizer.

use crate::bridge::canonicalize_expr;
use crate::model::{ViewNode, is_container, normalize_view_name};
use crate::tokenize::{Token, match_brace, match_paren};

use super::{KotlinRecognitionError, parse_body};

pub(crate) fn parse_if(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "(" {
        j += 1;
    }
    let mut depth = 0usize;
    let mut k = j;
    let mut cond = String::new();
    while k < tokens.len() {
        let t = &tokens[k].text;
        if t == "(" {
            depth += 1;
            if depth == 1 {
                k += 1;
                continue;
            }
        } else if t == ")" {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        cond.push_str(t);
        k += 1;
    }
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    let (then_branch, after_then) = parse_body(tokens, m)?;
    let mut i = after_then;
    let mut else_branch = Vec::new();
    if tokens.get(i).map(|t| t.text.as_str()) == Some("else") {
        let mut q = i + 1;
        while q < tokens.len() && tokens[q].text != "{" {
            q += 1;
        }
        let (els, after_else) = parse_body(tokens, q)?;
        else_branch = els;
        i = after_else;
    }
    Ok((
        ViewNode::If {
            cond: canonicalize_expr(cond.trim()),
            then_branch,
            else_branch,
        },
        i,
    ))
}

/// Parses `items(<coll>, key = <key>) { item -> … }`. The `item ->` body is
/// emitted empty by the codegen (FLUX-014); parity treats an empty body as the
/// expected, faithful shape.
pub(crate) fn parse_items(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "(" {
        j += 1;
    }
    let mut depth = 0usize;
    let mut k = j;
    let mut args = Vec::new();
    let mut cur = String::new();
    while k < tokens.len() {
        let t = &tokens[k].text;
        if t == "(" {
            depth += 1;
            if depth == 1 {
                k += 1;
                continue;
            }
        } else if t == ")" {
            depth -= 1;
            if depth == 0 {
                if !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
                break;
            }
        } else if t == "," && depth == 1 {
            args.push(std::mem::take(&mut cur));
            k += 1;
            continue;
        }
        cur.push_str(t);
        k += 1;
    }
    let collection = args
        .first()
        .map(|s| canonicalize_expr(s.trim()))
        .unwrap_or_default();
    let key_path = args
        .iter()
        .find_map(|a| {
            let a = a.trim();
            a.strip_prefix("key")
                .and_then(|r| r.strip_prefix('='))
                .map(str::trim)
        })
        .map(canonicalize_expr)
        .unwrap_or_default();
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    let (_body, end) = if m < tokens.len() {
        parse_body(tokens, m).unwrap_or_default()
    } else {
        (vec![], m + 1)
    };
    let _ = _body;
    Ok((
        ViewNode::ForEach {
            collection,
            key_path,
        },
        end,
    ))
}

/// Parses `NavHost(...) { … }` (the `Router` node).
pub(crate) fn parse_nav_host(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "{" {
        j += 1;
    }
    let (children, end) = parse_body(tokens, j)?;
    Ok((ViewNode::Router { children }, end))
}

/// Parses `composable("route") { … }` (the `Screen` node). The body's children
/// are the screen's content components.
pub(crate) fn parse_composable_dest(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "(" {
        j += 1;
    }
    let mut depth = 0usize;
    let mut k = j;
    let mut arg = String::new();
    while k < tokens.len() {
        let t = &tokens[k].text;
        if t == "(" {
            depth += 1;
            if depth == 1 {
                k += 1;
                continue;
            }
        } else if t == ")" {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        arg.push_str(t);
        k += 1;
    }
    let route = canonicalize_expr(arg.trim());
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    let (children, end) = parse_body(tokens, m)?;
    Ok((ViewNode::Screen { route, children }, end))
}

/// Parses a view expression `Name(...)` possibly with a `{ … }` trailing block —
/// e.g. `Text("…")`, `Column(...) { … }`, `Home()`.
pub(crate) fn parse_view(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let name = tokens[start].text.clone();
    let normalized = normalize_view_name(&name);
    let mut i = start + 1;
    // Skip the `( ... )` argument list (may contain closures — none affect
    // structure).
    if tokens.get(i).map(|t| t.text.as_str()) == Some("(") {
        let end = match_paren(tokens, i)
            .ok_or_else(|| KotlinRecognitionError(format!("unbalanced args in {normalized}")))?;
        i = end + 1;
    }
    if tokens.get(i).map(|t| t.text.as_str()) == Some("{") {
        // Container layouts (Column/Row/VStack/HStack/Stack) carry real structural
        // children. Every other adapter is a leaf: its trailing block is a codegen
        // placeholder (e.g. `Button(onClick = {}) { Text("") }`) and must not be
        // recovered as a child — the dev path models those adapters as childless.
        if is_container(&normalized) {
            let (children, after) = parse_body(tokens, i)?;
            return Ok((
                ViewNode::Primitive {
                    name: normalized,
                    props: vec![],
                    children,
                },
                after,
            ));
        }
        // Leaf adapter: consume the (empty or placeholder) block and stay childless.
        let end = match_brace(tokens, i)
            .ok_or_else(|| KotlinRecognitionError(format!("unbalanced body in {normalized}")))?;
        i = end + 1;
    }
    Ok((
        ViewNode::Primitive {
            name: normalized,
            props: vec![],
            children: vec![],
        },
        i,
    ))
}
