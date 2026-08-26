package dev.flux.host.wire

import java.io.ByteArrayOutputStream

/**
 * A small builder that emits frames byte-for-byte as [FrameDeserializer] reads
 * them (Appendix D §D.1/§D.3/§D.5/§D.9/§D.12). Used by the wire/host tests to
 * hand-build deterministic fixtures without a live dev server.
 *
 * The builder buffers each section and assembles the final frame in the exact
 * order [FrameDeserializer] consumes it, so the Init/Delta layouts always match
 * the decoder (and the Rust `flux-ir-serde` encoder).
 *
 * Two modes are supported:
 * - [init] — a full-tree `Init` frame (kind `0x02`). Call [node] for the root
 *   and every descendant, [stringEntry] for the literal string table,
 *   [componentSection] for component-name bindings, and [handlerSection] for
 *   handlers. The first [node] written is the root.
 * - [delta] — a `Delta` frame (kind `0x04`). Call the `patch*` helpers, then
 *   [stringEntry] and [handlerSection].
 */
class FrameBuilder {
    private enum class Mode { INIT, DELTA }

    private var mode: Mode = Mode.INIT
    private var seq: Int = 0
    private var deltaFlags: Int = 0

    private val nodes = ArrayList<ByteArray>()
    private val strings = ArrayList<Pair<UInt, String>>()
    private val components = ArrayList<Pair<UInt, String>>()
    private val seed = ArrayList<Pair<UInt, WireValue>>()
    private val sourceMap = ArrayList<Pair<UInt, String>>()
    private var blob: ByteArray = ByteArray(0)
    private val handlers = ArrayList<Pair<UInt, ClosureRef>>()
    private val patches = ArrayList<ByteArray>()

    // ── frame selection ──────────────────────────────────────────────────

    /** Begins a full-tree `Init` frame (kind `0x02`). */
    fun init(seq: Int = 0): FrameBuilder {
        mode = Mode.INIT
        this.seq = seq
        return this
    }

    /** Begins a `Delta` frame (kind `0x04`) with the given flags byte. */
    fun delta(
        seq: Int = 0,
        flags: Int = 0,
    ): FrameBuilder {
        mode = Mode.DELTA
        this.seq = seq
        this.deltaFlags = flags
        return this
    }

    // ── header primitives ─────────────────────────────────────────────────

    /** Writes the `FLUX` magic as a little-endian `u32` (`0x465C5558`). */
    fun magic() {
        // no-op marker kept for readability; magic is emitted by [build].
    }

    /** Sets the protocol version byte (default `1`). */
    fun version(v: Int) = Unit // emitted by build(); retained for call-site readability

    /** Sets the sequence number (emitted by [build] in the correct slot). */
    fun seq(v: Int): FrameBuilder {
        this.seq = v
        return this
    }

    /** Legacy alias for [init]/[delta]; kept so old call sites keep working. */
    fun flags(
        fullTree: Boolean,
        hasPure: Boolean = false,
    ): FrameBuilder {
        if (fullTree) {
            init(seq)
        } else {
            var f = 0
            if (hasPure) f = f or 0x20
            delta(seq, f)
        }
        return this
    }

    // The following legacy helpers are no-ops: the builder now derives every
    // section count from the buffered entries, so callers that previously
    // passed an explicit count need not change. Kept so existing tests compile.
    @Suppress("UNUSED_PARAMETER")
    fun patchCount(n: Int) = Unit

    @Suppress("UNUSED_PARAMETER")
    fun handlerCount(n: Int) = Unit

    @Suppress("UNUSED_PARAMETER")
    fun stringCount(n: Int) = Unit

    /** Legacy alias for [componentEntry]; adds each binding to the section. */
    fun componentSection(entries: List<Pair<UInt, String>> = emptyList()): FrameBuilder {
        for ((cid, name) in entries) componentEntry(cid, name)
        return this
    }

    // ── Init sections ─────────────────────────────────────────────────────

    /** Appends a node. The first node written becomes the root. */
    fun node(
        id: UInt,
        kind: UInt,
        component: UInt,
        props: List<Pair<UShort, WireValue>>,
        childIds: List<UInt>,
        pure: Boolean = false,
    ): FrameBuilder {
        val b = ByteArrayOutputStream()
        writeNode(b, id, kind, component, props, childIds, pure)
        nodes.add(b.toByteArray())
        return this
    }

    /** Adds a literal string-table entry (Appendix D §D.9). */
    fun stringEntry(
        id: UInt,
        text: String,
    ): FrameBuilder {
        strings.add(id to text)
        return this
    }

    /** Adds a component-name binding (separate id space from literals). */
    fun componentEntry(
        cid: UInt,
        name: String,
    ): FrameBuilder {
        components.add(cid to name)
        return this
    }

    /** Adds a state-seed cell `(signalId, value)`. */
    fun stateSeed(
        id: UInt,
        value: WireValue,
    ): FrameBuilder {
        seed.add(id to value)
        return this
    }

    /** Adds a source-map entry `(fileId, path)`. */
    fun sourceMapEntry(
        fileId: UInt,
        path: String,
    ): FrameBuilder {
        sourceMap.add(fileId to path)
        return this
    }

    // ── Delta sections ────────────────────────────────────────────────────

    /** Writes a `Replace` patch (tag `0x01`): a full node replacing `id`. */
    fun patchReplace(
        id: UInt,
        node: WireNodeBuilder,
    ): FrameBuilder {
        val b = ByteArrayOutputStream()
        b.write(0x01)
        u32(b, id.toInt())
        node.writeTo(b)
        patches.add(b.toByteArray())
        return this
    }

    /** Writes an `Update` patch (tag `0x02`): a prop diff on `id`. */
    fun patchUpdate(
        id: UInt,
        changes: List<Pair<UShort, WireValue>>,
        removals: List<UShort> = emptyList(),
    ): FrameBuilder {
        val b = ByteArrayOutputStream()
        b.write(0x02)
        u32(b, id.toInt())
        u16(b, changes.size)
        for ((idx, value) in changes) {
            u16(b, idx.toInt())
            writeValue(b, value)
        }
        u16(b, removals.size)
        for (r in removals) u16(b, r.toInt())
        patches.add(b.toByteArray())
        return this
    }

    /** Writes an `Insert` patch (tag `0x03`): `node` placed under parent. */
    fun patchInsert(
        parentId: UInt,
        index: Int,
        node: WireNodeBuilder,
    ): FrameBuilder {
        val b = ByteArrayOutputStream()
        b.write(0x03)
        u32(b, parentId.toInt())
        u16(b, index)
        node.writeTo(b)
        patches.add(b.toByteArray())
        return this
    }

    /** Writes a `Remove` patch (tag `0x04`) for `id`. */
    fun patchRemove(id: UInt): FrameBuilder {
        val b = ByteArrayOutputStream()
        b.write(0x04)
        u32(b, id.toInt())
        patches.add(b.toByteArray())
        return this
    }

    /** Legacy `Insert` overload matching the old `FrameBuilder` signature. */
    fun patchInsert(
        parentId: UInt,
        index: Int,
        id: UInt,
        kind: UInt,
        component: UInt,
        props: List<Pair<UShort, WireValue>>,
        childIds: List<UInt>,
    ): FrameBuilder = patchInsert(parentId, index, wireNode(id, kind, component, props, childIds))

    /** Writes a `Reorder` patch (tag `0x05`): `keys` reordered under parent. */
    fun patchReorder(
        parentId: UInt,
        keys: List<UInt>,
    ): FrameBuilder {
        val b = ByteArrayOutputStream()
        b.write(0x05)
        u32(b, parentId.toInt())
        u16(b, keys.size)
        for (k in keys) u32(b, k.toInt())
        patches.add(b.toByteArray())
        return this
    }

    /** Writes a `Handler` patch (tag `0x06`): closure `ref` bound to `id`. */
    fun patchHandler(
        id: UInt,
        ref: ClosureRef,
    ): FrameBuilder {
        val b = ByteArrayOutputStream()
        b.write(0x06)
        u32(b, id.toInt())
        writeClosureRef(b, ref)
        patches.add(b.toByteArray())
        return this
    }

    // ── shared handler section ─────────────────────────────────────────────

    /** Sets the shared bytecode blob and `HandlerDef` stream. */
    fun handlerSection(
        blobBytes: ByteArray,
        handlers: List<Pair<UInt, ClosureRef>>,
    ): FrameBuilder {
        blob = blobBytes
        this.handlers.clear()
        this.handlers.addAll(handlers)
        return this
    }

    // ── assembly ────────────────────────────────────────────────────────────

    /** Assembles and returns the frame bytes matching the decoder contract. */
    fun build(): ByteArray {
        val out = ByteArrayOutputStream()
        // Shared 6-byte header: magic(4) | version(1) | kind(1).
        out.write(0x58)
        out.write(0x55)
        out.write(0x5C)
        out.write(0x46) // 0x465C5558 LE
        out.write(0x01) // version
        when (mode) {
            Mode.INIT -> buildInit(out)
            Mode.DELTA -> buildDelta(out)
        }
        return out.toByteArray()
    }

    private fun buildInit(out: ByteArrayOutputStream) {
        out.write(0x02) // FRAME_INIT
        u32(out, seq)
        // Root node, then a u32 count of extra (descendant) nodes.
        require(nodes.isNotEmpty()) { "Init frame requires at least a root node" }
        out.writeBytes(nodes[0])
        u32(out, nodes.size - 1)
        for (i in 1 until nodes.size) out.writeBytes(nodes[i])
        // signal state_seed: u16 count + (u32 id, value).
        u16(out, seed.size)
        for ((id, value) in seed) {
            u32(out, id.toInt())
            writeValue(out, value)
        }
        // source_map: u16 count + (u32 fileId, u16 len + utf8 path).
        u16(out, sourceMap.size)
        for ((fid, path) in sourceMap) {
            u32(out, fid.toInt())
            encodeStr(out, path)
        }
        // literal string table (u32 count).
        u32(out, strings.size)
        for ((id, text) in strings) {
            u32(out, id.toInt())
            encodeStr(out, text)
        }
        // component-name section (u16 count).
        u16(out, components.size)
        for ((cid, name) in components) {
            u32(out, cid.toInt())
            encodeStr(out, name)
        }
        // handler section: blob (u32 len + bytes) + u16 count + HandlerDefs.
        writeHandlerSection(out)
        // ADR-0027 signal_meta presence marker: 0 = none for fixtures.
        out.write(0)
    }

    private fun buildDelta(out: ByteArrayOutputStream) {
        out.write(0x04) // FRAME_DELTA
        u32(out, seq)
        out.write(deltaFlags and 0xFF)
        u16(out, patches.size)
        u16(out, handlers.size)
        u16(out, strings.size)
        for (p in patches) out.writeBytes(p)
        for ((id, text) in strings) {
            u32(out, id.toInt())
            encodeStr(out, text)
        }
        writeHandlerSection(out)
        // ADR-0027 signal_meta: present only when FLAG_NODE_HAS_SIGNAL_DEPS set.
        if ((deltaFlags and 0x40) != 0) out.write(0) else out.write(0)
    }

    private fun writeHandlerSection(out: ByteArrayOutputStream) {
        u32(out, blob.size)
        out.writeBytes(blob)
        u16(out, handlers.size)
        for ((id, ref) in handlers) {
            u32(out, id.toInt())
            writeClosureRef(out, ref)
        }
    }

    // ── node / value writers ────────────────────────────────────────────────

    private fun writeNode(
        b: ByteArrayOutputStream,
        id: UInt,
        kind: UInt,
        component: UInt,
        props: List<Pair<UShort, WireValue>>,
        childIds: List<UInt>,
        pure: Boolean = false,
    ) {
        u32(b, id.toInt())
        // Pure flag is carried in bit 0x20 of the kind byte (Appendix D §D.4),
        // matching the Rust encoder's `kind | PURE_FLAG`.
        b.write((kind.toInt() or if (pure) 0x20 else 0) and 0xFF)
        u32(b, component.toInt())
        u16(b, props.size)
        for ((idx, value) in props) {
            u16(b, idx.toInt())
            writeValue(b, value)
        }
        u16(b, childIds.size)
        for (cid in childIds) {
            b.write(0x01) // Node child tag
            u32(b, cid.toInt())
        }
        u16(b, 0) // handler count
        // span: file/start/end (u32 each)
        u32(b, 0)
        u32(b, 0)
        u32(b, 0)
    }

    private fun writeValue(
        b: ByteArrayOutputStream,
        value: WireValue,
    ) {
        when (value) {
            WireValue.Null -> b.write(0x00)
            is WireValue.IntVal -> {
                b.write(0x01)
                i64(b, value.value)
            }
            is WireValue.FloatVal -> {
                b.write(0x02)
                i64(b, java.lang.Double.doubleToRawLongBits(value.value))
            }
            is WireValue.BoolVal -> {
                b.write(0x03)
                b.write(if (value.value) 1 else 0)
            }
            is WireValue.StrVal -> {
                b.write(0x04)
                u32(b, value.id.toInt())
            }
            is WireValue.HandlerRefVal -> {
                b.write(0x05)
                u32(b, value.handlerId.toInt())
            }
            is WireValue.ListVal -> {
                b.write(0x06)
                u16(b, value.items.size)
                value.items.forEach { writeValue(b, it) }
            }
            is WireValue.RecordVal -> {
                b.write(0x07)
                u16(b, value.fields.size)
                value.fields.forEach { (idx, v) ->
                    u16(b, idx.toInt())
                    writeValue(b, v)
                }
            }
        }
    }

    private fun writeClosureRef(
        b: ByteArrayOutputStream,
        ref: ClosureRef,
    ) {
        for (byte in ref.hash) b.write(byte.toInt() and 0xFF)
        u32(b, ref.bytecodeOffset.toInt())
        u16(b, ref.bytecodeLen.toInt())
        u16(b, ref.signals.size)
        for (s in ref.signals) u32(b, s.toInt())
        u32(b, 0)
        u32(b, 0)
        u32(b, 0) // span file/start/end
    }

    private fun encodeStr(
        b: ByteArrayOutputStream,
        s: String,
    ) {
        val bytes = s.toByteArray(Charsets.UTF_8)
        u16(b, bytes.size)
        b.writeBytes(bytes)
    }

    private fun u8(
        b: ByteArrayOutputStream,
        v: Int,
    ) = b.write(v and 0xFF)

    private fun u16(
        b: ByteArrayOutputStream,
        v: Int,
    ) {
        b.write(v and 0xFF)
        b.write((v ushr 8) and 0xFF)
    }

    private fun u32(
        b: ByteArrayOutputStream,
        v: Int,
    ) {
        b.write(v and 0xFF)
        b.write((v ushr 8) and 0xFF)
        b.write((v ushr 16) and 0xFF)
        b.write((v ushr 24) and 0xFF)
    }

    private fun i64(
        b: ByteArrayOutputStream,
        v: Long,
    ) {
        b.write((v and 0xFF).toInt())
        b.write(((v ushr 8) and 0xFF).toInt())
        b.write(((v ushr 16) and 0xFF).toInt())
        b.write(((v ushr 24) and 0xFF).toInt())
        b.write(((v ushr 32) and 0xFF).toInt())
        b.write(((v ushr 40) and 0xFF).toInt())
        b.write(((v ushr 48) and 0xFF).toInt())
        b.write(((v ushr 56) and 0xFF).toInt())
    }
}

/**
 * Fluent builder for a [WireNode]-shaped node inside a `Replace`/`Insert` patch.
 * Mirrors [FrameBuilder.writeNode] without allocating a [WireNode].
 */
class WireNodeBuilder(
    private val id: UInt,
    private val kind: UInt,
    private val component: UInt,
    private val props: List<Pair<UShort, WireValue>>,
    private val childIds: List<UInt>,
    private val pure: Boolean = false,
) {
    internal fun writeTo(b: ByteArrayOutputStream) = encode(b)

    private fun encode(b: ByteArrayOutputStream) {
        u32(b, id.toInt())
        b.write((kind.toInt() or if (pure) 0x20 else 0) and 0xFF)
        u32(b, component.toInt())
        u16(b, props.size)
        for ((idx, value) in props) {
            u16(b, idx.toInt())
            writeValue(b, value)
        }
        u16(b, childIds.size)
        for (cid in childIds) {
            b.write(0x01)
            u32(b, cid.toInt())
        }
        u16(b, 0)
        u32(b, 0)
        u32(b, 0)
        u32(b, 0)
    }

    private fun u16(
        b: ByteArrayOutputStream,
        v: Int,
    ) {
        b.write(v and 0xFF)
        b.write((v ushr 8) and 0xFF)
    }

    private fun u32(
        b: ByteArrayOutputStream,
        v: Int,
    ) {
        b.write(v and 0xFF)
        b.write((v ushr 8) and 0xFF)
        b.write((v ushr 16) and 0xFF)
        b.write((v ushr 24) and 0xFF)
    }

    private fun writeValue(
        b: ByteArrayOutputStream,
        value: WireValue,
    ) {
        when (value) {
            WireValue.Null -> b.write(0x00)
            is WireValue.IntVal -> {
                b.write(0x01)
                i64(b, value.value)
            }
            is WireValue.FloatVal -> {
                b.write(0x02)
                i64(b, java.lang.Double.doubleToRawLongBits(value.value))
            }
            is WireValue.BoolVal -> {
                b.write(0x03)
                b.write(if (value.value) 1 else 0)
            }
            is WireValue.StrVal -> {
                b.write(0x04)
                u32(b, value.id.toInt())
            }
            is WireValue.HandlerRefVal -> {
                b.write(0x05)
                u32(b, value.handlerId.toInt())
            }
            is WireValue.ListVal -> {
                b.write(0x06)
                u16(b, value.items.size)
                value.items.forEach { writeValue(b, it) }
            }
            is WireValue.RecordVal -> {
                b.write(0x07)
                u16(b, value.fields.size)
                value.fields.forEach { (idx, v) ->
                    u16(b, idx.toInt())
                    writeValue(b, v)
                }
            }
        }
    }

    private fun i64(
        b: ByteArrayOutputStream,
        v: Long,
    ) {
        b.write((v and 0xFF).toInt())
        b.write(((v ushr 8) and 0xFF).toInt())
        b.write(((v ushr 16) and 0xFF).toInt())
        b.write(((v ushr 24) and 0xFF).toInt())
        b.write(((v ushr 32) and 0xFF).toInt())
        b.write(((v ushr 40) and 0xFF).toInt())
        b.write(((v ushr 48) and 0xFF).toInt())
        b.write(((v ushr 56) and 0xFF).toInt())
    }
}

/** Creates a [WireNodeBuilder] for use with the `patch*` helpers. */
fun wireNode(
    id: UInt,
    kind: UInt,
    component: UInt,
    props: List<Pair<UShort, WireValue>> = emptyList(),
    childIds: List<UInt> = emptyList(),
    pure: Boolean = false,
): WireNodeBuilder = WireNodeBuilder(id, kind, component, props, childIds, pure)
