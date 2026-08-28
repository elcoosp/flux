//  ShadowTree.swift
//  Native shadow-tree model (Appendix C §C.1 + Appendix D §D.3).
//
//  A `ShadowNode` is the host-side mirror of an IR node. It carries a stable
//  `NodeId`, a `NodeKind`, the component id, props, children and handler ids.
//  The keyed reconciler (FLUX-006 scope item 5) keeps node identities stable
//  across frames so native views are mutated in place rather than recreated.

import Foundation

/// The kind of an IR node (Appendix D §D.3 / Appendix C).
enum NodeKind: UInt8, Equatable, Sendable {
    case component = 0
    case primitive = 1
    case forEach = 2
    case `if` = 3
    case match = 4
    case router = 5
    case screen = 6
}

/// A source span, carried through frames so VM errors can be reported against
/// the originating `.flux` location (Appendix E §E.6).
struct FluxSpan: Equatable, Sendable {
    let fileId: UInt32
    let start: UInt32
    let end: UInt32
}

/// A key used by the reconciler to match children across frames. `Node` keys are
/// the `NodeId`; `Splice` keys are explicit `u64` keys from the IR (Appendix D §D.4).
enum Child: Equatable, Sendable {
    case node(UInt32)
    case splice(itemCount: UInt16, items: [(key: UInt64, node: UInt32)])

    static func == (lhs: Child, rhs: Child) -> Bool {
        switch (lhs, rhs) {
        case let (.node(a), .node(b)): a == b
        case let (.splice(ac, ai), .splice(bc, bi)):
            ac == bc && ai.count == bi.count
                && zip(ai, bi).allSatisfy { $0.key == $1.key && $0.node == $1.node }
        case (.node, _), (.splice, _): false
        }
    }
}

/// A single prop: a `PropIdx` (Appendix C) paired with its value.
struct Prop: Equatable, Sendable {
    let index: UInt16
    let value: FluxValue
}

/// A deserialized node from an `Init` or `Replace`/`Insert` frame.
struct ShadowNode: Equatable, Sendable {
    let id: UInt32
    let kind: NodeKind
    let componentId: UInt32
    let props: [Prop]
    let childCount: UInt16
    let children: [Child]
    let handlerCount: UInt16
    let handlers: [UInt32]
    let span: FluxSpan
    /// Handler id bound to this node's `onMount` block (§18.4), evaluated once
    /// when the node is first built. `nil` when the component declares none.
    let mountHandler: UInt32?
    /// Handler id bound to this node's `onCleanup` block (§18.4), evaluated when
    /// the node is removed from the tree. `nil` when none is declared.
    let cleanupHandler: UInt32?
    /// Marks the node as `@pure` (§18.10): its subtree depends only on its own
    /// resolved props, so when those props' content hash is unchanged the
    /// reconciler skips re-reconciling the subtree entirely.
    let isPure: Bool

    /// Creates a node, defaulting lifecycle hooks and purity to absent.
    init(
        id: UInt32,
        kind: NodeKind,
        componentId: UInt32,
        props: [Prop],
        childCount: UInt16,
        children: [Child],
        handlerCount: UInt16,
        handlers: [UInt32],
        span: FluxSpan,
        mountHandler: UInt32? = nil,
        cleanupHandler: UInt32? = nil,
        isPure: Bool = false
    ) {
        self.id = id
        self.kind = kind
        self.componentId = componentId
        self.props = props
        self.childCount = childCount
        self.children = children
        self.handlerCount = handlerCount
        self.handlers = handlers
        self.span = span
        self.mountHandler = mountHandler
        self.cleanupHandler = cleanupHandler
        self.isPure = isPure
    }
}

extension ShadowNode {
    /// Looks up a prop value by index, or `nil` if absent.
    func prop(_ index: UInt16) -> FluxValue? {
        props.first { $0.index == index }?.value
    }
}

/// A closure reference from a `Handler` patch (Appendix D §D.7).
struct ClosureRef: Equatable, Sendable {
    let hash: [UInt8] // 8-byte BLAKE3 content hash
    let bytecodeOffset: UInt32
    let bytecodeLen: UInt16
    let signalCount: UInt16
    let signals: [UInt32]
    let span: FluxSpan
}
