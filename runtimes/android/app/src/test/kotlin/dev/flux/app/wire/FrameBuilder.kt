package dev.flux.app.wire

/**
 * A small builder that emits frames byte-for-byte as [FrameDeserializer] reads
 * them (Appendix D §D.1/§D.3/§D.5). Used by [FrameDeserializerTest] to hand-build
 * deterministic fixtures without a live dev server.
 */
class FrameBuilder {
    private val out = ArrayList<Byte>()

    fun magic() {
        // "FLUX" little-endian: bytes F(0x46) L(0x4C) U(0x55) X(0x58).
        out.add(0x46.toByte())
        out.add(0x4C.toByte())
        out.add(0x55.toByte())
        out.add(0x58.toByte())
    }

    fun version(v: Int) = out.add(v.toByte())

    fun seq(v: Int) = u32(v)

    fun flags(
        fullTree: Boolean,
        error: Boolean = false,
        heartbeat: Boolean = false,
        hasState: Boolean = false,
    ) {
        var f = 0
        if (fullTree) f = f or 0x01
        if (error) f = f or 0x02
        if (heartbeat) f = f or 0x04
        if (hasState) f = f or 0x08
        out.add(f.toByte())
    }

    fun patchCount(n: Int) = u16(n)

    fun handlerCount(n: Int) = u16(n)

    fun stringCount(n: Int) = u16(n)

    /**
     * Writes one string-table entry (Appendix D §D.9) as `(id, utf8 text)`.
     * Call these after [stringCount] and before the root [node] in a full-tree
     * frame, matching the order [FrameDeserializer] reads them.
     */
    fun stringEntry(
        id: UInt,
        text: String,
    ) {
        u32(id.toInt())
        val bytes = text.toByteArray(Charsets.UTF_8)
        u16(bytes.size)
        out.addAll(bytes.toList())
    }

    /**
     * Writes an `Update` patch (Appendix D §D.2, tag 0x02): a prop diff applied
     * to the node [id].
     */
    fun patchUpdate(
        id: UInt,
        changes: List<Pair<UShort, WireValue>>,
        removals: List<UShort> = emptyList(),
    ) {
        out.add(0x02)
        u32(id.toInt())
        u16(changes.size)
        for ((idx, value) in changes) {
            u16(idx.toInt())
            writeValue(value)
        }
        u16(removals.size)
        for (r in removals) u16(r.toInt())
    }

    /**
     * Writes an `Insert` patch (Appendix D §D.2, tag 0x03): a new [node] placed
     * at [index] under [parentId]. The inserted node is self-contained (its own
     * children are not decoded by the host from this patch).
     */
    fun patchInsert(
        parentId: UInt,
        index: Int,
        id: UInt,
        kind: UInt,
        component: UInt,
        props: List<Pair<UShort, WireValue>>,
        childIds: List<UInt>,
    ) {
        out.add(0x03)
        u32(parentId.toInt())
        u16(index)
        node(id, kind, component, props, childIds)
    }

    /** Writes a `Remove` patch (Appendix D §D.2, tag 0x04) for node [id]. */
    fun patchRemove(id: UInt) {
        out.add(0x04)
        u32(id.toInt())
    }

    fun node(
        id: UInt,
        kind: UInt,
        component: UInt,
        props: List<Pair<UShort, WireValue>>,
        childIds: List<UInt>,
    ) {
        u32(id.toInt())
        out.add(kind.toByte())
        u32(component.toInt())
        u16(props.size)
        for ((idx, value) in props) {
            u16(idx.toInt())
            writeValue(value)
        }
        u16(childIds.size)
        for (cid in childIds) {
            out.add(0x01) // Node child tag
            u32(cid.toInt())
        }
        u16(0) // handler count
        u32(0)
        u32(0)
        u32(0) // span
    }

    fun build(): ByteArray = out.toByteArray()

    private fun u8(v: Int) = out.add((v and 0xFF).toByte())

    private fun u16(v: Int) {
        out.add((v and 0xFF).toByte())
        out.add(((v ushr 8) and 0xFF).toByte())
    }

    private fun u32(v: Int) {
        out.add((v and 0xFF).toByte())
        out.add(((v ushr 8) and 0xFF).toByte())
        out.add(((v ushr 16) and 0xFF).toByte())
        out.add(((v ushr 24) and 0xFF).toByte())
    }

    private fun writeValue(value: WireValue) {
        when (value) {
            WireValue.Null -> out.add(0x00)
            is WireValue.IntVal -> {
                out.add(0x01)
                i64(value.value)
            }
            is WireValue.FloatVal -> {
                out.add(0x02)
                i64(java.lang.Double.doubleToRawLongBits(value.value))
            }
            is WireValue.BoolVal -> {
                out.add(0x03)
                out.add(if (value.value) 1 else 0)
            }
            is WireValue.StrVal -> {
                out.add(0x04)
                u32(value.id.toInt())
            }
            is WireValue.HandlerRefVal -> {
                out.add(0x05)
                u32(value.handlerId.toInt())
            }
            is WireValue.ListVal -> {
                out.add(0x06)
                u16(value.items.size)
                value.items.forEach { writeValue(it) }
            }
            is WireValue.RecordVal -> {
                out.add(0x07)
                u16(value.fields.size)
                value.fields.forEach { (idx, v) ->
                    u16(idx.toInt())
                    writeValue(v)
                }
            }
        }
    }

    private fun i64(v: Long) {
        out.add((v and 0xFF).toByte())
        out.add(((v ushr 8) and 0xFF).toByte())
        out.add(((v ushr 16) and 0xFF).toByte())
        out.add(((v ushr 24) and 0xFF).toByte())
        out.add(((v ushr 32) and 0xFF).toByte())
        out.add(((v ushr 40) and 0xFF).toByte())
        out.add(((v ushr 48) and 0xFF).toByte())
        out.add(((v ushr 56) and 0xFF).toByte())
    }
}
