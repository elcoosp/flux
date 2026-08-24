//  WireModels.swift
//  Decoded representations of Flux wire frames (Appendix D).
//
//  These are the host-side view of a frame after `FrameDeserializer` has walked
//  the byte stream. The `ShadowNode` model lives in `ShadowTree.swift`; the wire
//  layer only knows how to decode bytes into these structures.

/// A single patch operation (Appendix D §D.2).
enum Patch: Equatable, Sendable {
    case replace(id: UInt32, node: ShadowNode)
    case update(id: UInt32, changes: [Prop], removals: [UInt16])
    case insert(parentId: UInt32, index: UInt16, node: ShadowNode)
    case remove(id: UInt32)
    case reorder(parentId: UInt32, keys: [UInt32])
    case handler(id: UInt32, closure: ClosureRef)
}

/// A handler definition (Appendix D §D.8): a handler id plus its closure.
struct HandlerDef: Equatable, Sendable {
    let handlerId: UInt32
    let closure: ClosureRef
}

/// An interned string entry (Appendix D §D.9).
struct StringEntry: Equatable, Sendable {
    let stringId: UInt32
    let value: String
}

/// A file mapping in a source-map delta (Appendix D §D.11).
struct FileEntry: Equatable, Sendable {
    let fileId: UInt32
    let path: String
}

/// A state-cell seed carried by an Init or delta frame (Appendix D §D.10).
struct StateCell: Equatable, Sendable {
    let signalId: UInt32
    let value: VMValue
}

/// A fully decoded frame. `full` frames (Init) carry a root node; patch frames
/// carry `patches`/`handlers`/`strings` deltas.
///
/// `nodes` is the flat id → `ShadowNode` table for the whole tree. The wire
/// represents children by id (Appendix D §D.4); the host keeps every reachable
/// node here so the reconciler can resolve child ids to their definitions and
/// reconcile by stable `NodeId` without re-decoding the tree.
struct FluxFrame: Equatable, Sendable {
    let version: UInt8
    let seq: UInt32
    let flags: UInt8
    let root: ShadowNode?
    let nodes: [UInt32: ShadowNode]
    let patches: [Patch]
    let handlers: [HandlerDef]
    let strings: [StringEntry]
    let state: [StateCell]
    let files: [FileEntry]
}
