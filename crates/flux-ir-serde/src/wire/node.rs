//! `NodeRef` wire codec (Appendix D §D.3).

use flux_syntax::{NodeKind, NodeRef};

use super::WireError;
use super::child::{decode_child, encode_child};
use super::cursor::Reader;
use super::props::{decode_props, encode_props};
use super::span::{decode_span, encode_span};

pub(crate) fn encode_node(w: &mut super::cursor::Writer, node: &NodeRef) {
    w.u32(node.id);
    w.u8(node.kind.tag());
    w.u32(node.component_id);
    encode_props(w, &node.props);
    w.u16(node.children.len() as u16);
    for child in &node.children {
        encode_child(w, child);
    }
    w.u16(node.handlers.len() as u16);
    for handler in &node.handlers {
        w.u32(*handler);
    }
    encode_span(w, &node.span);
}

pub(crate) fn decode_node(r: &mut Reader<'_>) -> Result<NodeRef, WireError> {
    let id = r.u32("node.id")?;
    let kind_tag = r.u8("node.kind")?;
    let kind = NodeKind::from_tag(kind_tag).ok_or(WireError::InvalidTag {
        tag: kind_tag,
        context: "node.kind",
        at: r.pos() - 1,
    })?;
    let component_id = r.u32("node.component_id")?;
    let props = decode_props(r)?;
    let child_count = r.u16("node.child_count")?;
    r.ensure_capacity(child_count as usize, "node.children")?;
    let mut children = Vec::with_capacity(child_count as usize);
    for _ in 0..child_count {
        children.push(decode_child(r)?);
    }
    let handler_count = r.u16("node.handler_count")?;
    r.ensure_capacity(handler_count as usize, "node.handlers")?;
    let mut handlers = Vec::with_capacity(handler_count as usize);
    for _ in 0..handler_count {
        handlers.push(r.u32("node.handler")?);
    }
    let span = decode_span(r)?;
    Ok(NodeRef {
        id,
        kind,
        component_id,
        props,
        children,
        handlers,
        span,
    })
}
