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
    /// Re-bind the live instance behind `old` to `new`, preserving its signal
    /// state, refs and scroll/focus (roadmap Phase 3). Emitted instead of
    /// `.replace` when a structural edit changed a node's identity (e.g. `Column`
    /// → `Row`) but the node still denotes the same component at the same
    /// position. The host re-keys the instance and applies `node` to it — it
    /// never destroys and re-materialises the subtree, which would reset state.
    case reattach(old: UInt32, new: UInt32, node: ShadowNode)
}

/// A handler definition (Appendix D §D.8): a handler id plus its closure.
struct HandlerDef: Equatable, Sendable {
    let handlerId: UInt32
    let closure: ClosureRef
    /// The handler's bytecode, sliced from the frame's shared handler blob
    /// (Appendix D §D.12 handler section). `nil` only when the frame carried no
    /// handler section (e.g. a delta with `handler_count == 0`); such handlers
    /// cannot be dispatched.
    let bytecode: [UInt8]?
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
    let value: FluxValue
}

/// Signal-graph metadata for a single node (ADR-0027 §T13/T14).
///
/// Captured by `emit_signal_metadata` during lowering: `deps` is the set of
/// signals the node reads, and `thunk` (when present) is the bytecode body that
/// re-materialises the node's dynamic props against the live signal graph. The
/// `layout` maps each prop expression's ordinal position in the thunk's result
/// `Record` to the on-wire `PropIdx` it must be stored under.
struct NodeSignalMeta: Equatable, Sendable {
    let deps: [UInt32]
    let thunk: ClosureRef?
    let layout: [UInt16]
}

/// A server-side compile/type error delivered via an `Error` (0x03) frame
/// (Appendix D §D.12.3). The Rust encoder writes only `message` and an optional
/// `span`; there is intentionally no diagnostics array on the wire.
///
/// `message` is `public` so the host app can render it from a separate module;
/// `span` is internal to the decoder and exposed to consumers via `location`.
public struct ServerError: Equatable, Sendable {
    /// Human-readable error message from the dev server.
    public let message: String
    /// Source span where the error occurred, if the server supplied one.
    let span: FluxSpan?

    /// A human-readable location string (`"at byte offset … (file …)"`), or
    /// `nil` when the server sent no span. Computed within the module so the
    /// (internal) `FluxSpan` type never crosses the module boundary.
    public var location: String? {
        guard let span else { return nil }
        return "at byte offset \(span.start)…\(span.end) (file \(span.fileId))"
    }
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
    /// Component-name bindings (Appendix D §D.9): each node's `componentId`
    /// maps to its adapter name ("Text", "Column", ...). Kept SEPARATE from
    /// `strings` so a component id never collides with a prop string id in the
    /// string resolver — the reconciler resolves components via `componentNames`
    /// and prop strings via `strings`. This mirrors the Android host, which feeds
    /// `componentNames` to the adapter registry rather than the string table.
    let componentNames: [StringEntry]
    /// Per-node signal-graph metadata (ADR-0027 §T13/T14): the signals each node
    /// reads and, for dynamic nodes, the prop-thunk closure that re-materialises
    /// re-materialises its props against the live signal graph.
    let signalMeta: [UInt32: NodeSignalMeta]
    /// Server-side compile/type error delivered via an `Error` (0x03) frame.
    /// `nil` on normal frames; when present the executor surfaces it as a banner
    /// and leaves the last good tree intact (Appendix E §E.6), so a failed
    /// recompile does not blank the screen.
    let error: ServerError?
    /// `true` for housekeeping frames (`Heartbeat` 0x05, `InternString` 0x07,
    /// `StringInterned` 0x08) that carry no tree mutation. The executor
    /// short-circuits these before touching the live tree or clearing prior
    /// fault state.
    let isControl: Bool

    /// Designated initializer. The two trailing parameters default so existing
    /// call sites (which omit them) keep compiling; the `decode` functions and
    /// tests pass them explicitly. An explicit initializer is used instead of
    /// relying on the synthesized memberwise initializer so the defaulted
    /// `error`/`isControl` parameters are unambiguously part of the API surface
    /// across build-cache states.
    init(
        version: UInt8,
        seq: UInt32,
        flags: UInt8,
        root: ShadowNode?,
        nodes: [UInt32: ShadowNode],
        patches: [Patch],
        handlers: [HandlerDef],
        strings: [StringEntry],
        state: [StateCell],
        files: [FileEntry],
        componentNames: [StringEntry],
        signalMeta: [UInt32: NodeSignalMeta],
        error: ServerError? = nil,
        isControl: Bool = false
    ) {
        self.version = version
        self.seq = seq
        self.flags = flags
        self.root = root
        self.nodes = nodes
        self.patches = patches
        self.handlers = handlers
        self.strings = strings
        self.state = state
        self.files = files
        self.componentNames = componentNames
        self.signalMeta = signalMeta
        self.error = error
        self.isControl = isControl
    }
}
