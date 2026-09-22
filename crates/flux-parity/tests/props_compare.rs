//! T-501 regression tests: the parity harness must compare view props, not
//! just structure. The AST reducer, Swift recognizer, and Kotlin recognizer
//! must all surface the same canonical prop subset so dev-vs-release prop
//! drift is caught.
//!
//! Recognized props: `label`, `color`, `alignment`.
//! `text` is excluded because codegen renders it positionally or as a child
//! Text node — it never survives as a named arg in generated source.

use flux_parity::compile;
use flux_parity::from_ast;
use flux_parity::ViewNode;

/// Helper: find the first Primitive node with the given name in a subtree
/// and return its props.
fn find_props<'a>(nodes: &'a [ViewNode], name: &str) -> Option<&'a [(String, String)]> {
    for node in nodes {
        match node {
            ViewNode::Primitive {
                name: n,
                props,
                children,
            } if n == name => return Some(props),
            ViewNode::Primitive { children, .. } => {
                if let Some(found) = find_props(children, name) {
                    return Some(found);
                }
            }
            ViewNode::Component { children, .. } => {
                if let Some(found) = find_props(children, name) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// Helper: parse + type-check + AST-reduce a `.flux` source fragment.
fn from_src(src: &str) -> Vec<ViewNode> {
    let (ast, _, _) = compile(src, 1).expect("compile succeeded");
    from_ast(&ast)
}

/// The dev-side AST reducer extracts `label` as a named arg.
#[test]
fn label_prop_surfaces_in_dev_tree() {
    let src = "compo Hello\n  Button(label: \"Save\", onPress: {})";
    let dev = from_src(src);
    let props = find_props(&dev, "Button").expect("Button node exists");
    assert!(
        props.iter().any(|(k, v)| k == "label" && v == "\"Save\""),
        "label prop should be extracted: got {:?}",
        props
    );
}

/// The recognized prop subset excludes `text` — it is rendered positionally
/// (Text/Image) or as a child Text node (Button), never as a named arg
/// in generated source.
#[test]
fn text_prop_is_not_extracted() {
    let src = "compo Hello\n  Button(text: \"Save\", onPress: {})";
    let dev = from_src(src);
    let props = find_props(&dev, "Button").expect("Button node exists");
    assert!(
        props.is_empty(),
        "text prop should not be in recognized subset: got {:?}",
        props
    );
}

/// A `color` prop on Text is extracted on the dev side.
#[test]
fn color_prop_surfaces_in_dev_tree() {
    let src = "compo Hello\n  Text(\"Hi\", color: \"red\")";
    let dev = from_src(src);
    let props = find_props(&dev, "Text").expect("Text node exists");
    let has_color = props.iter().any(|(k, _)| k == "color");
    assert!(has_color, "color prop should be extracted from dev AST");
}

/// An unrecognized prop like `width` must NOT be extracted.
#[test]
fn unrecognized_prop_is_not_extracted() {
    let src = "compo Hello\n  Text(\"Hi\", width: 100)";
    let dev = from_src(src);
    let props = find_props(&dev, "Text").expect("Text node exists");
    assert!(
        props.iter().all(|(k, _)| k != "width"),
        "width should not be in recognized subset: got {:?}",
        props
    );
}
