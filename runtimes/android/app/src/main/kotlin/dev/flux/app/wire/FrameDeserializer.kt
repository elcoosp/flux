package dev.flux.app.wire

/**
 * Binary frame parser for the Flux wire protocol (Appendix D).
 *
 * Decodes the shared `FLUX` header (§D.1), the node tree (§D.3), values (§D.5),
 * patches (§D.2), string entries (§D.9), and state delta (§D.10). The result is
 * an immutable [Frame] that the runtime applies to the [dev.flux.app.shadow.ShadowTree].
 *
 * Decoding is total: a malformed buffer raises [WireError] rather than
 * producing a half-built tree, so the host can show a red error overlay instead
 * of crashing (Appendix E §E.6).
 *
 * The frame layout follows Appendix D exactly:
 * `magic(4) version(1) seq(4) flags(1) patch_count(2) handler_count(2)
 * string_count(2) [patches] [handlers] [strings] [state_delta]`.
 * For a full-tree (bit 0 set) frame the root node follows the string section in
 * place of patches; the Init frame (§D.12.2) carries `root + state_seed`.
 */
public object FrameDeserializer {
    /** The little-endian magic `"FLUX"` (Appendix D §D.1). */
    public const val MAGIC: UInt = 0x58554C46u

    /** Decodes [bytes] into a [Frame], or raises [WireError] on a malformed frame. */
    public fun deserialize(bytes: ByteArray): Frame {
        val r = ByteReader(bytes)
        val magic = r.u32().toUInt()
        if (magic != MAGIC) {
            throw WireError("bad magic 0x%08X (expected 0x%08X)".format(magic.toLong(), MAGIC.toLong()))
        }
        val version = r.u8().toUByte()
        val seq = r.u32().toUInt()
        val flags = r.u8()
        val fullTree = (flags and 0x01) != 0
        val patchCount = r.u16()
        val handlerCount = r.u16()
        val stringCount = r.u16()

        val patches = ArrayList<Patch>(patchCount)
        repeat(patchCount) { patches.add(decodePatch(r)) }
        // handler_count reserves space for closure defs; in the MLP host the
        // handler bodies arrive via Patch 0x06 (ClosureRef), so the header's
        // handler_count entries carry no body and are skipped here.
        repeat(handlerCount) { /* no body in this frame layout */ }

        val strings = ArrayList<StringEntry>(stringCount)
        repeat(stringCount) { strings.add(decodeStringEntry(r)) }

        val stateDelta = if ((flags and 0x08) != 0) decodeStateDelta(r) else emptyList()

        val (root, extraNodes) =
            if (fullTree) {
                val rootNode = decodeNode(r)
                val extras = ArrayList<WireNode>()
                while (r.has(1)) extras.add(decodeNode(r))
                rootNode to extras
            } else {
                null to emptyList<WireNode>()
            }

        return Frame(version, seq, fullTree, patches, root, strings, stateDelta, extraNodes)
    }

    private fun decodePatch(r: ByteReader): Patch {
        val tag = r.u8().toUByte()
        return when (tag.toInt()) {
            0x01 ->
                Patch( // Replace: u32 id, Node
                    tag,
                    id = r.u32().toUInt(),
                    parentId = 0u,
                    index = 0u,
                    node = decodeNode(r),
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            0x02 -> { // Update: u32 id, PropDiff
                val id = r.u32().toUInt()
                Patch(
                    tag,
                    id = id,
                    parentId = 0u,
                    index = 0u,
                    node = null,
                    diff = decodePropDiff(r),
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            }
            0x03 -> { // Insert: u32 parent_id, u16 index, Node
                val parent = r.u32().toUInt()
                val index = r.u16().toUShort()
                Patch(
                    tag,
                    id = 0u,
                    parentId = parent,
                    index = index,
                    node = decodeNode(r),
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            }
            0x04 ->
                Patch( // Remove: u32 id
                    tag,
                    id = r.u32().toUInt(),
                    parentId = 0u,
                    index = 0u,
                    node = null,
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            0x05 -> { // Reorder: u32 parent_id, u16 key_count, [u32; key_count]
                val parent = r.u32().toUInt()
                val keyCount = r.u16().toUShort()
                val keys = ArrayList<UInt>(keyCount.toInt())
                repeat(keyCount.toInt()) { keys.add(r.u32().toUInt()) }
                Patch(
                    tag,
                    id = 0u,
                    parentId = parent,
                    index = 0u,
                    node = null,
                    diff = null,
                    keyCount = keyCount,
                    keys = keys,
                    closure = null,
                )
            }
            0x06 -> { // Handler: u32 id, ClosureRef
                val id = r.u32().toUInt()
                Patch(
                    tag,
                    id = id,
                    parentId = 0u,
                    index = 0u,
                    node = null,
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = decodeClosureRef(r),
                )
            }
            else -> throw WireError("unknown patch tag ${tag.toInt()} at offset ${r.position}")
        }
    }

    private fun decodePropDiff(r: ByteReader): PropDiff {
        val changeCount = r.u16()
        val changes = ArrayList<Pair<UShort, WireValue>>(changeCount)
        repeat(changeCount) {
            val idx = r.u16().toUShort()
            changes.add(idx to decodeValue(r))
        }
        val removalCount = r.u16()
        val removals = ArrayList<UShort>(removalCount)
        repeat(removalCount) { removals.add(r.u16().toUShort()) }
        return PropDiff(changes, removals)
    }

    private fun decodeClosureRef(r: ByteReader): ClosureRef {
        val hash = r.bytes(8)
        val offset = r.u32().toUInt()
        val len = r.u16().toUShort()
        val signalCount = r.u16()
        val signals = ArrayList<UInt>(signalCount)
        repeat(signalCount) { signals.add(r.u32().toUInt()) }
        r.u32()
        r.u32()
        r.u32() // span_file/start/end ignored by host
        return ClosureRef(hash, offset, len, signals)
    }

    private fun decodeStringEntry(r: ByteReader): StringEntry {
        val id = r.u32().toUInt()
        val len = r.u16()
        val text = r.utf8(len)
        return StringEntry(id, text)
    }

    private fun decodeStateDelta(r: ByteReader): List<Pair<UInt, WireValue>> {
        val cellCount = r.u16()
        val cells = ArrayList<Pair<UInt, WireValue>>(cellCount)
        repeat(cellCount) {
            val id = r.u32().toUInt()
            cells.add(id to decodeValue(r))
        }
        return cells
    }

    private fun decodeNode(r: ByteReader): WireNode {
        val id = r.u32().toUInt()
        val kindByte = r.u8()
        // Resolve the wire kind byte to an adapter registry key. The MLP host
        // maps well-known component kinds to their string tag; unknown bytes
        // fall back to the decimal string so a test frame's custom kinds still
        // resolve. Real component-name resolution (via the string table) lands
        // in FLUX-016.
        val kind = kindAlias(kindByte) ?: kindByte.toString()
        val componentId = r.u32().toUInt()
        val propCount = r.u16()
        val props = ArrayList<Pair<UShort, WireValue>>(propCount)
        repeat(propCount) {
            val idx = r.u16().toUShort()
            props.add(idx to decodeValue(r))
        }
        val childCount = r.u16()
        val children = ArrayList<WireChild>(childCount)
        repeat(childCount) { children.add(decodeChild(r)) }
        val handlerCount = r.u16()
        val handlerIds = ArrayList<UInt>(handlerCount)
        repeat(handlerCount) { handlerIds.add(r.u32().toUInt()) }
        val spanFile = r.u32().toUInt()
        val spanStart = r.u32().toUInt()
        val spanEnd = r.u32().toUInt()
        return WireNode(id, kind, componentId, props, children, handlerIds, spanFile, spanStart, spanEnd)
    }

    /**
     * Maps a wire node-kind byte to an adapter registry key. Returns `null` for
     * unrecognized bytes (caller falls back to the decimal string). The seven
     * dev adapter kinds (FLUX-009) are the only well-known mappings the MLP
     * host resolves without a string table.
     */
    private fun kindAlias(byte: Int): String? =
        when (byte) {
            0x10 -> "text"
            0x11 -> "button"
            0x12 -> "column"
            0x13 -> "row"
            0x14 -> "text_field"
            0x15 -> "screen"
            0x16 -> "router"
            else -> null
        }

    private fun decodeChild(r: ByteReader): WireChild =
        when (val tag = r.u8()) {
            0x01 -> WireChild.Node(r.u32().toUInt())
            0x02 -> {
                val itemCount = r.u16()
                val items = ArrayList<Pair<ULong, UInt>>(itemCount)
                repeat(itemCount) {
                    val key = r.u32().toULong() // u64 key (low 32 read)
                    r.u32() // high 32 of key (disjoint in MLP host: key is u32)
                    items.add(key to r.u32().toUInt())
                }
                WireChild.Splice(items)
            }
            else -> throw WireError("unknown child tag $tag at offset ${r.position}")
        }

    private fun decodeValue(r: ByteReader): WireValue =
        when (val tag = r.u8()) {
            0x00 -> WireValue.Null
            0x01 -> WireValue.IntVal(r.i64())
            0x02 -> WireValue.FloatVal(r.f64())
            0x03 -> WireValue.BoolVal(r.u8() != 0)
            0x04 -> WireValue.StrVal(r.u32().toUInt())
            0x05 -> WireValue.HandlerRefVal(r.u32().toUInt())
            0x06 -> {
                val count = r.u16()
                val items = ArrayList<WireValue>(count)
                repeat(count) { items.add(decodeValue(r)) }
                WireValue.ListVal(items)
            }
            0x07 -> {
                val count = r.u16()
                val fields = ArrayList<WireValue.RecordVal.Field>(count)
                repeat(count) {
                    val idx = r.u16().toUShort()
                    fields.add(WireValue.RecordVal.Field(idx, decodeValue(r)))
                }
                WireValue.RecordVal(fields)
            }
            else -> throw WireError("unknown value tag $tag at offset ${r.position}")
        }
}
