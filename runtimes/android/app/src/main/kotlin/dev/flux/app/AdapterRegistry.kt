package dev.flux.app

import dev.flux.ui.FluxAdapter
import dev.flux.ui.FluxNativeView
import dev.flux.ui.FluxUiKit

/**
 * Maps an interned `ComponentId` to the dev adapter that builds its native
 * view (FLUX-017).
 *
 * The wire `Init` frame carries a string-table delta that maps component-name
 * ids to their kind tags (`"text"`, `"column"`, …). Resolution is therefore a
 * two-hop lookup: `componentId → kind → adapter`. Anchoring on `ComponentId`
 * (rather than the raw wire kind byte) means the same adapter kit serves
 * components declared under arbitrary names in the stdlib and user code, and a
 * missing component id degrades to `null` instead of crashing the host
 * (Appendix E §E.6).
 *
 * @property byComponent the `ComponentId → kind tag` mapping decoded from the
 *   `Init` frame's string table.
 * @property byKind the kit's `kind tag → adapter` map, re-exported from
 *   [FluxUiKit] so resolution never reaches into the kit's internals.
 */
public class AdapterRegistry(
    private val byComponent: Map<UInt, String>,
    private val byKind: Map<String, FluxAdapter<out FluxNativeView>> = FluxUiKit.adapters,
) {
    /**
     * Resolves [componentId] to its dev adapter, or `null` when no component
     * with that id was declared in the `Init` frame.
     */
    public fun resolve(componentId: UInt): FluxAdapter<out FluxNativeView>? {
        val kind = byComponent[componentId] ?: return null
        return byKind[kind]
    }

    /**
     * Returns the dev adapter registered for [kind], or `null` when no adapter
     * handles that kind tag.
     */
    public fun adapterForKind(kind: String): FluxAdapter<out FluxNativeView>? = byKind[kind]

    /** The set of kind tags the kit can render. */
    public fun kinds(): Set<String> = byKind.keys

    /**
     * Returns a copy of this registry extended with [entries], as when a new
     * `Init` frame delivers an updated string-table delta. Existing ids are
     * last-wins overwritten; the kit's `kind → adapter` map is shared (not
     * copied) so the two registries resolve identically.
     */
    public fun withEntries(entries: Iterable<StringTableEntry>): AdapterRegistry {
        val merged = byComponent.toMutableMap()
        for (entry in entries) merged[entry.id] = entry.text
        return AdapterRegistry(merged, byKind)
    }

    public companion object {
        /**
         * Builds a registry from an `Init` frame's string-table entries.
         *
         * Each [StringTableEntry] binds a `ComponentId` to the kind tag its
         * adapter is keyed by in [FluxUiKit]. Duplicate ids are last-wins; an
         * entry whose text is not a known adapter kind is simply never hit by
         * [resolve] (the lookup falls through to `null`).
         *
         * @param entries the `(ComponentId, kind tag)` pairs from the Init frame.
         */
        public fun fromStringTable(entries: Iterable<StringTableEntry>): AdapterRegistry {
            val byComponent = entries.associate { it.id to it.text }
            return AdapterRegistry(byComponent)
        }
    }
}

/**
 * A single `(ComponentId, kind tag)` binding delivered in an `Init` frame's
 * string-table delta (Appendix D §D.9 / §D.12.2).
 *
 * @property id the interned component-name id.
 * @property text the adapter kind tag this component renders as (e.g. `"text"`).
 */
public data class StringTableEntry(
    val id: UInt,
    val text: String,
)
