package dev.flux.host.vm

/**
 * Resolves an interned `StringId` (carried inside [FluxValue.StrVal]) to the
 * string it names, so the VM can implement `STR_LEN` / `STR_CONCAT` over real
 * text rather than over the numeric id (Appendix E §E.1).
 *
 * The wire frame's string table (Appendix D §D.9) is the source of truth; the
 * executor builds a [TableStringResolver] from each frame's string entries and
 * threads it into [FluxBytecodeVM.run]. When no table is available the golden
 * ISA vectors (and the `flux-vm-ref` oracle) assume a deterministic proxy with
 * no live table: [DecimalStringResolver] provides exactly that proxy, so
 * conformance is preserved without inventing a table.
 */
public interface StringResolver {
    /** Returns the text for [id], or its decimal string when unbound. */
    public fun resolve(id: UInt): String

    /**
     * Returns the interned id of the concatenation of [x] and [y].
     *
     * Without a live table the MLP host cannot grow the frame's string table at
     * runtime (dynamic intern is out of scope per ADR-flux-0028); the default
     * resolver reproduces the `flux-vm-ref` oracle's `x*10_000_000 + y` proxy so
     * the golden vectors stay green, while [TableStringResolver] widens the
     * proxy to a deterministic hash of the real joined text.
     */
    public fun concat(
        x: UInt,
        y: UInt,
    ): UInt = (resolve(x).toUInt() * 10_000_000u) + resolve(y).toUInt()
}

/**
 * The default resolver used by the conformance suite and any run with no string
 * table: an id resolves to its own decimal digits, and `concat` reproduces the
 * `flux-vm-ref` oracle's `x*10_000_000 + y` proxy. Mirrors the Rust oracle's
 * str handling, so `STR_LEN`/`STR_CONCAT` agree with the golden vectors.
 */
public object DecimalStringResolver : StringResolver {
    override fun resolve(id: UInt): String = id.toString()
}

/**
 * A resolver backed by an explicit `id → text` table built from a frame's
 * string entries (Appendix D §D.9). Unbound ids degrade to their decimal
 * string so the VM never panics on a missing entry.
 *
 * @property table the `StringId → text` mapping delivered by the most recent frame.
 */
public class TableStringResolver(
    private val table: Map<UInt, String>,
) : StringResolver {
    override fun resolve(id: UInt): String = table[id] ?: id.toString()

    override fun concat(
        x: UInt,
        y: UInt,
    ): UInt {
        val joined = resolve(x) + resolve(y)
        var h: UInt = 0xcbf29ce4u
        for (b in joined.toByteArray(Charsets.UTF_8)) {
            h = (h xor b.toUInt()) * 0x1000193u
        }
        // Bias away from small ids that collide with the frame's literal table.
        return 0x8000_0000u or (h and 0x7FFF_FFFFu)
    }
}
