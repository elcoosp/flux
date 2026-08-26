package dev.flux.host.wire

/**
 * A reverse index from a resolved string back to its canonical wire `StringId`.
 *
 * The MLP host carries strings as interned `u32` ids on the wire, but the
 * adapter kit resolves them to real `String` values. When a native event
 * payload is dispatched back into the VM, the resolved string must map to the
 * *same* id the handler expects — the canonical table id, in O(1) by string
 * lookup (perf task 7, P2). A per-event `hashCode()` is fast but unstable and
 * never matches the table, so we keep an explicit `String → UInt` map.
 *
 * A string not present in the table is **not** given a synthetic hash here. A
 * synthetic id silently bypasses interning and reintroduces the brittleness 4d
 * was raised to remove; instead, the missing string must be interned through the
 * dev server via [dev.flux.host.FluxExecutor.internString], which returns a
 * canonical id (`< STRING_ID_CANONICAL_CEILING`). [resolve] therefore returns the
 * canonical id when the string is known and otherwise a deterministic non-canonical
 * fallback id so dispatch still terminates — the *authoritative* path is always
 * the `InternString` RPC.
 */
public class StringInterning(
    private val table: Map<String, UInt>,
) {
    /** Returns the canonical id for [str], or a deterministic fallback id. */
    public fun resolve(str: String): UInt = table[str] ?: fallbackId(str)

    /**
     * Returns a copy of this index with [str] bound to [id]. Used by the executor
     * to cache a server-interned id (brittleness 4d) so a later dispatch of the
     * same text resolves in O(1) and canonically.
     */
    public fun with(
        str: String,
        id: UInt,
    ): StringInterning = StringInterning(table.toMutableMap().apply { put(str, id) })

    /** Deterministic, non-canonical fallback id (high half) for an uninterned string. */
    private fun fallbackId(str: String): UInt {
        var h: UInt = 0x811c9dc5u
        for (b in str.toByteArray(Charsets.UTF_8)) {
            h = (h xor b.toUInt()) * 0x1000193u
        }
        // Biased into the high half so it is distinguishable from a canonical
        // wire id (`< STRING_ID_CANONICAL_CEILING`) and so [FluxExecutor.internString]
        // routes an uninterned string to the dev server rather than returning this
        // local proxy as if it were canonical. This is never placed on the wire as
        // a canonical id — the canonical id always comes from the server (brittleness 4d).
        return STRING_ID_CANONICAL_CEILING or (h and 0x7FFF_FFFFu)
    }

    public companion object {
        /** Builds the index from a frame's [StringEntry] stream. */
        public fun fromEntries(entries: Iterable<StringEntry>): StringInterning {
            val map = LinkedHashMap<String, UInt>()
            for (e in entries) map[e.text] = e.id
            return StringInterning(map)
        }

        /** An empty index (every string resolves to a synthetic id). */
        public fun empty(): StringInterning = StringInterning(emptyMap())
    }
}
