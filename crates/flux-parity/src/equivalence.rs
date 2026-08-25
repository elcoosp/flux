//! Structural-equivalence relation for [`ViewNode`] trees.
//!
//! Reduces faithful-but-textually-different shapes (condition whitespace, route
//! quotes, synthetic layout wrappers, `if`/`when` branch ordering) so that the
//! dev-path AST tree and the two release-path codegen trees compare equal.

use crate::model::{ViewNode, is_container};

/// Structural parity between two reduced view trees.
///
/// Most nodes are compared structurally and positionally. The following
/// faithfully-equivalent-but-textually-different cases are normalized so the
/// dev lowerer and the release codegen compare equal:
///
/// * **Condition whitespace.** The dev `SurfaceBridge` and the codegen
///   `expressions` backend emit the same expression with slightly different
///   spacing (e.g. `0 == "ios"` vs `0 == "ios" `). Conditions are compared
///   after collapsing runs of whitespace.
/// * **Route quotes.** The dev path canonicalizes a string literal route as
///   `"home"` (with the quotes as part of the value) whereas codegen emits the
///   bare route `home`; leading/trailing quotes are stripped before comparison.
/// * **Synthetic layout wrappers.** The dev lowerer wraps a component body in a
///   single-child `Column`/`Row`; the release codegen emits the body's children
///   directly at the component root. A layout container with exactly one child is
///   a structural pass-through, so it is elided before comparison.
/// * **`If` branch ordering.** The dev lowerer represents `when … otherwise`
///   as a single `If` whose `then`/`else` children are folded into one node,
///   whereas the release codegen splits them into a Swift `if … else` / Kotlin
///   `if (…) … else`. The combined set of branch subtrees is therefore compared
///   as an unordered bag.
pub(crate) fn structurally_equal(a: &[ViewNode], b: &[ViewNode]) -> bool {
    let a = elide_wrappers(a);
    let b = elide_wrappers(b);
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(&b).all(|(x, y)| node_equal(x, y))
}

/// Removes synthetic layout-wrapper containers (structural pass-throughs) from a
/// sibling list. This models two faithful-but-textually-different shapes:
///
/// * A component body wrapped in a single `Column`/`Row` in the dev lowerer while
///   the codegen emits the children directly at the component root — when the
///   whole sibling list is exactly one layout container, it is descended into.
/// * A layout container with a single child in one path but the child inlined in
///   the other — the container is likewise descended into.
///
/// Recurses so wrappers at any depth are normalized consistently.
fn elide_wrappers(nodes: &[ViewNode]) -> Vec<ViewNode> {
    if nodes.len() == 1 {
        if let ViewNode::Primitive { name, children, .. } = &nodes[0] {
            if is_container(name) {
                return elide_wrappers(children);
            }
        }
    }
    nodes
        .iter()
        .map(|n| map_children(n, elide_wrappers))
        .collect()
}

/// Recursively rebuilds a node, applying `f` to each sibling list. Used to apply
/// a transformation (such as [`elide_wrappers`]) uniformly across a whole tree.
fn map_children(node: &ViewNode, f: impl Fn(&[ViewNode]) -> Vec<ViewNode> + Copy) -> ViewNode {
    match node {
        ViewNode::Component { name, children } => ViewNode::Component {
            name: name.clone(),
            children: f(children),
        },
        ViewNode::Primitive { name, children, .. } => ViewNode::Primitive {
            name: name.clone(),
            props: vec![],
            children: f(children),
        },
        ViewNode::If {
            cond,
            then_branch,
            else_branch,
        } => ViewNode::If {
            cond: cond.clone(),
            then_branch: f(then_branch),
            else_branch: f(else_branch),
        },
        ViewNode::ForEach {
            collection,
            key_path,
        } => ViewNode::ForEach {
            collection: collection.clone(),
            key_path: key_path.clone(),
        },
        ViewNode::Match { scrutinee, arms } => ViewNode::Match {
            scrutinee: scrutinee.clone(),
            arms: arms.iter().map(|(p, c)| (p.clone(), f(c))).collect(),
        },
        ViewNode::Router { children } => ViewNode::Router {
            children: f(children),
        },
        ViewNode::Screen { route, children } => ViewNode::Screen {
            route: route.clone(),
            children: f(children),
        },
    }
}

/// Collapses runs of whitespace and removes `/* … */` block comments so the dev
/// path's `(/* unsupported expr */ 0 == "ios")` and the codegen path's
/// `( /* unsupported expr */ 0 == "ios" )` compare equal.
fn norm_cond(s: &str) -> String {
    let without_comments: String = s
        .chars()
        .scan(false, |in_comment, c| {
            if *in_comment {
                if c == '/' {
                    *in_comment = false;
                }
                Some(None)
            } else if c == '/' {
                *in_comment = true;
                Some(None)
            } else {
                Some(Some(c))
            }
        })
        .flatten()
        .collect();
    without_comments.split_whitespace().collect::<String>()
}

/// Normalizes a ForEach key path so the three paths compare equal. The dev
/// lowerer emits key:.self / key:.id; Swift emits key: .self / key: .id
/// (literal backslash); Kotlin emits {it} / {it.id}. All are the same
/// identity selector, so the key:/{}//. wrappers are removed and the Kotlin
/// it aliases are mapped onto the dev/Swift self spelling.
fn norm_key(s: &str) -> String {
    let trimmed = s
        .trim()
        .trim_start_matches("key:")
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim()
        .trim_start_matches('.');
    match trimmed {
        "it" => "self",
        "it.id" => "id",
        other => other,
    }
    .to_owned()
}

/// Strips surrounding double-quotes from a route literal, repeating until stable
/// so the dev path's quoted route value and the codegen path's bare route compare.
fn strip_quotes(s: &str) -> String {
    let mut t = s.trim().to_owned();
    while t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t = t[1..t.len() - 1].to_owned();
    }
    t
}

fn node_equal(a: &ViewNode, b: &ViewNode) -> bool {
    match (a, b) {
        (
            ViewNode::Component {
                name: n1,
                children: c1,
            },
            ViewNode::Component {
                name: n2,
                children: c2,
            },
        ) => n1 == n2 && structurally_equal(c1, c2),
        (
            ViewNode::Primitive {
                name: n1,
                children: c1,
                ..
            },
            ViewNode::Primitive {
                name: n2,
                children: c2,
                ..
            },
        ) => n1 == n2 && structurally_equal(c1, c2),
        (
            ViewNode::If {
                cond: c1,
                then_branch: t1,
                else_branch: e1,
            },
            ViewNode::If {
                cond: c2,
                then_branch: t2,
                else_branch: e2,
            },
        ) => norm_cond(c1) == norm_cond(c2) && branch_bag_equal(t1, e1, t2, e2),
        (
            ViewNode::ForEach {
                collection: c1,
                key_path: k1,
            },
            ViewNode::ForEach {
                collection: c2,
                key_path: k2,
            },
        ) => c1 == c2 && norm_key(k1) == norm_key(k2),
        (
            ViewNode::Match {
                scrutinee: s1,
                arms: a1,
            },
            ViewNode::Match {
                scrutinee: s2,
                arms: a2,
            },
        ) => s1 == s2 && arms_equal(a1, a2),
        (ViewNode::Router { children: c1 }, ViewNode::Router { children: c2 }) => {
            structurally_equal(c1, c2)
        }
        (
            ViewNode::Screen {
                route: r1,
                children: c1,
            },
            ViewNode::Screen {
                route: r2,
                children: c2,
            },
        ) => strip_quotes(r1) == strip_quotes(r2) && structurally_equal(c1, c2),
        _ => false,
    }
}

/// Compares the combined branch contents of two `If` nodes as an unordered bag.
/// See [`structurally_equal`] for why `then`/`else` ordering is not significant.
fn branch_bag_equal(t1: &[ViewNode], e1: &[ViewNode], t2: &[ViewNode], e2: &[ViewNode]) -> bool {
    let left = elide_wrappers(t1)
        .into_iter()
        .chain(elide_wrappers(e1))
        .collect::<Vec<_>>();
    let right = elide_wrappers(t2)
        .into_iter()
        .chain(elide_wrappers(e2))
        .collect::<Vec<_>>();
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(&right).all(|(x, y)| node_equal(x, y))
}

fn arms_equal(a: &[(String, Vec<ViewNode>)], b: &[(String, Vec<ViewNode>)]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b)
        .all(|((la, ca), (lb, cb))| la == lb && structurally_equal(ca, cb))
}
