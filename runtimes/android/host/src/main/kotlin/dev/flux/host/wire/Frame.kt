package dev.flux.host.wire

import dev.flux.host.vm.FluxValue

/**
 * Wire `Value` encoding (Appendix D §D.5): tagged unions carrying exactly the
 * payloads [dev.flux.host.vm.FluxValue] does. The deserializer converts these
 * into [FluxValue] once a frame is fully parsed; the two types are kept separate
 * so the wire layer never leaks into the VM.
 */
public sealed interface WireValue {
    public data object Null : WireValue

    public data class IntVal(
        val value: Long,
    ) : WireValue

    public data class FloatVal(
        val value: Double,
    ) : WireValue

    public data class BoolVal(
        val value: Boolean,
    ) : WireValue

    public data class StrVal(
        val id: UInt,
    ) : WireValue

    public data class HandlerRefVal(
        val handlerId: UInt,
    ) : WireValue

    public data class ListVal(
        val items: List<WireValue>,
    ) : WireValue

    public data class RecordVal(
        val fields: List<Field>,
    ) : WireValue {
        public data class Field(
            val index: UShort,
            val value: WireValue,
        )
    }
}

/**
 * Wire `Node` encoding (Appendix D §D.3).
 *
 * @property id the node id.
 * @property kind the node-kind tag, resolved to the adapter registry key
 *   (e.g. `"text"`, `"column"` for dev adapters). The wire encodes this as a
 *   `UByte` (Appendix D §D.3); the deserializer converts it to the string the
 *   [dev.flux.host.shadow.ShadowTree] looks up. For the MLP host a missing
 *   alias falls back to the decimal byte string.
 * @property componentId the interned component name id.
 * @property props decoded props as `(prop_idx, WireValue)` pairs.
 * @property children decoded child encodings.
 * @property handlerIds handler closure ids bound on this node.
 * @property isPure `true` when the component was declared `@pure` (§18.10);
 *   the reconciler skips the subtree when its props are referentially equal.
 * @property spanFile source file id for diagnostics.
 * @property spanStart byte offset of the node in source.
 * @property spanEnd byte offset of the node end in source.
 */
public data class WireNode(
    val id: UInt,
    val kind: String,
    val componentId: UInt,
    val props: List<Pair<UShort, WireValue>>,
    val children: List<WireChild>,
    val handlerIds: List<UInt>,
    val isPure: Boolean = false,
    val spanFile: UInt,
    val spanStart: UInt,
    val spanEnd: UInt,
)

/**
 * Wire `Child` encoding (Appendix D §D.4).
 *
 * @property nodeId for a `Node` child (tag 0x01).
 * @property items for a `Splice` child (tag 0x02): the ordered `(key, nodeId)`.
 */
public sealed interface WireChild {
    public val nodeId: UInt

    public data class Node(
        val id: UInt,
    ) : WireChild {
        override val nodeId: UInt get() = id
    }

    public data class Splice(
        val items: List<Pair<ULong, UInt>>,
    ) : WireChild {
        override val nodeId: UInt get() = items.firstOrNull()?.second ?: 0u
    }
}

/**
 * A decoded wire frame (Appendix D §D.1 + D.12.2 Init).
 *
 * The deserializer produces this immutable snapshot; the runtime applies it to
 * the [ShadowTree]. Only the fields the MLP host consumes are modeled here.
 *
 * @property version protocol version byte.
 * @property seq monotonic sequence number.
 * @property fullTree `true` for a full-tree frame, `false` for a delta.
 * @property patches the patch entries (delta frames only).
 * @property root the root node (full-tree Init frames only).
 * @property strings newly interned strings (string-table delta).
 * @property componentNames `ComponentId → component-name` bindings (Appendix D
 *   §D.9), a SEPARATE id space from [strings]. The registry resolves each node's
 *   adapter from these; the string resolver must NOT see them (a `ComponentId`
 *   and a literal `StringId` can share a numeric value, which would corrupt
 *   resolution if merged).
 * @property stateDelta initial/updated signal cells.
 * @property handlers frame-level handler definitions (Appendix D §D.8, Gap G1).
 * @property bytecodeBlob the shared handler-bytecode blob (Appendix D §D.12)
 *   that [handlers]' closures index into.
 * @property extraNodes descendant nodes of [root], flat after the root in a
 *   full-tree frame. Children are referenced by id; the runtime resolves them
 *   from `root + extraNodes` (Appendix D §D.12.2 Init carries the whole tree).
 */
public data class Frame(
    val version: UByte,
    val seq: UInt,
    val fullTree: Boolean,
    val patches: List<Patch>,
    val root: WireNode?,
    val strings: List<StringEntry>,
    val componentNames: List<StringEntry> = emptyList(),
    val stateDelta: List<Pair<UInt, WireValue>>,
    val handlers: List<HandlerDef> = emptyList(),
    val bytecodeBlob: BytecodeBlob? = null,
    val extraNodes: List<WireNode> = emptyList(),
    /** Per-node ADR-0027 signal-graph metadata, keyed by node id (Appendix D §T13). */
    val signalMeta: Map<UInt, NodeSignalMeta> = emptyMap(),
)

/**
 * Per-node ADR-0027 (FA-IRWIRE) signal-graph metadata (Appendix D §T13/T14).
 *
 * @property deps the distinct signal ids the node reads.
 * @property thunk the prop-thunk closure that re-materialises the node's dynamic
 *   props against the live signal graph, or `null` for control-only/static nodes.
 * @property layout maps each prop expression's ordinal position in the thunk's
 *   result `Record` to the on-wire `PropIdx`.
 */
public data class NodeSignalMeta(
    val deps: List<UInt>,
    val thunk: ClosureRef?,
    val layout: List<UShort>,
)

/**
 * A single patch entry (Appendix D §D.2).
 *
 * @property tag the patch tag (0x01 Replace … 0x06 Handler).
 * @property id the affected node id.
 * @property parentId the parent node id (Insert/Reorder).
 * @property index the insertion index (Insert).
 * @property node the replacement node (Replace/Insert).
 * @property diff the prop diff (Update).
 * @property keyCount / [keys] for Reorder.
 * @property closure the closure reference (Handler).
 */
public data class Patch(
    val tag: UByte,
    val id: UInt,
    val parentId: UInt,
    val index: UShort,
    val node: WireNode?,
    val diff: PropDiff?,
    val keyCount: UShort,
    val keys: List<UInt>,
    val closure: ClosureRef?,
)

/** A `PropDiff` (Appendix D §D.6). */
public data class PropDiff(
    val changes: List<Pair<UShort, WireValue>>,
    val removals: List<UShort>,
)

/** A `ClosureRef` (Appendix D §D.7). */
public data class ClosureRef(
    val hash: ByteArray,
    val bytecodeOffset: UInt,
    val bytecodeLen: UShort,
    val signals: List<UInt>,
)

/**
 * A decoded handler definition (Appendix D §D.8): a handler id plus the
 * closure whose bytecode is sliced from the frame's shared blob by
 * `bytecodeOffset`/`bytecodeLen`.
 *
 * Unlike a patch-bound `ClosureRef` (which the shadow tree attaches to a node),
 * a `HandlerDef` is the frame-level transport for handler bodies: the executor
 * slices each one out of [Frame.bytecodeBlob] and registers it for dispatch
 * (Gap G1, §3.1 task 1).
 *
 * @property handlerId the closure-table index this definition populates.
 * @property closure the closure descriptor indexing [Frame.bytecodeBlob].
 */
public data class HandlerDef(
    val handlerId: UInt,
    val closure: ClosureRef,
)

/**
 * The shared handler-bytecode blob (Appendix D §D.12 handler section).
 *
 * Every [HandlerDef.closure] indexes this single buffer by `bytecodeOffset`
 * (u32) / `bytecodeLen` (u16); the executor slices each handler's bytecode out
 * of it when registering handlers. An empty blob means the frame carries no
 * handlers.
 *
 * The blob is stored as a *window* `(data, offset, len)` over the frame's
 * backing buffer rather than a freshly copied array (perf task 8, P2), so a
 * frame decode allocates nothing for the handler section — the executor's
 * single `copyOfRange` per handler is the only copy.
 *
 * @property data the frame's backing buffer.
 * @property offset the blob's start within [data].
 * @property len the blob's byte length.
 */
public data class BytecodeBlob(
    val data: ByteArray,
    val offset: Int,
    val len: Int,
)

/** A `StringEntry` (Appendix D §D.9). */
public data class StringEntry(
    val id: UInt,
    val text: String,
)
