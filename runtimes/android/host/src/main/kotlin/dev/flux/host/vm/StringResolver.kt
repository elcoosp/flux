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
     * proxy to a deterministic hash of the real joined text. The host never
     * treats these as canonical ids — a string produced here must be interned
     * through the dev server ([dev.flux.host.FluxExecutor.internString]) before
     * it crosses the wire (brittleness 4d).
     */
    public fun concat(
        x: UInt,
        y: UInt,
    ): UInt = (resolve(x).toUInt() * 10_000_000u) + resolve(y).toUInt()

    /**
     * Returns the interned id of [text], the inverse of [resolve].
     *
     * `TO_STRING` (0xD0, ADR-0043) renders a value to text and must bind it to a
     * fresh `StringId` so downstream prop resolution observes a real string.
     * Without a live table the host cannot grow the frame's string table at
     * runtime (dynamic intern is out of scope per ADR-flux-0028); the default
     * resolver reproduces the `flux-vm-ref` oracle's FNV-1a proxy so the golden
     * vectors stay green, while [TableStringResolver] widens the proxy to a
     * deterministic hash of the real text. The result is **not** a canonical id
     * and must be interned via the dev server before it reaches the wire.
     */
    public fun intern(text: String): UInt {
        var h: UInt = 0x811c9dc5u
        for (b in text.toByteArray(Charsets.UTF_8)) {
            h = (h xor b.toUInt()) * 0x1000193u
        }
        return h
    }
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
    // Runtime-produced text (concatenations, `TO_STRING` renders) that is not
    // part of the frame's literal string table. Keyed by the deterministic proxy
    // id computed in [concat]/[intern] so [resolve] returns the *real* text
    // (and therefore the *real* length under `STR_LEN`) rather than a decimal
    // proxy. This is a host-internal cache, never a canonical wire id — a string
    // here must still be interned through the dev server (brittleness 4d) before
    // it crosses the wire.
    private val extra = LinkedHashMap<UInt, String>()

    override fun resolve(id: UInt): String = extra[id] ?: table[id] ?: id.toString()

    override fun concat(
        x: UInt,
        y: UInt,
    ): UInt {
        val joined = resolve(x) + resolve(y)
        val id = internHash(joined)
        extra[id] = joined
        return id
    }

    override fun intern(text: String): UInt {
        val id = internHash(text)
        extra[id] = text
        return id
    }

    /** Deterministic, non-canonical proxy for [text] (FNV-1a). */
    private fun internHash(text: String): UInt {
        var h: UInt = 0x811c9dc5u
        for (b in text.toByteArray(Charsets.UTF_8)) {
            h = (h xor b.toUInt()) * 0x1000193u
        }
        return h
    }

    /**
     * Returns a copy of this resolver with [text] bound to [id]. Used by the
     * executor to cache a server-interned string (brittleness 4d) so subsequent
     * `STR_LEN`/`STR_CONCAT`/`TO_STRING` resolutions observe the canonical id.
     * Runtime-produced [extra] entries are carried forward so a prior
     * concatenation still resolves to real text.
     */
    public fun with(
        text: String,
        id: UInt,
    ): TableStringResolver {
        val next = table.toMutableMap().apply { put(id, text) }
        val resolver = TableStringResolver(next)
        resolver.extra.putAll(extra)
        return resolver
    }
}
