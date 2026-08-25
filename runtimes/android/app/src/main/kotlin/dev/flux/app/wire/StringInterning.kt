package dev.flux.app.wire

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
 * Strings not present in the table (e.g. dynamically produced values) get a
 * deterministic synthetic id derived from the string, so dispatch stays stable
 * within a session without growing the wire table.
 */
public class StringInterning(
    private val table: Map<String, UInt>,
) {
    /** Returns the canonical id for [str], or a deterministic synthetic id. */
    public fun resolve(str: String): UInt = table[str] ?: syntheticId(str)

    private fun syntheticId(str: String): UInt = str.hashCode().toUInt()

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
