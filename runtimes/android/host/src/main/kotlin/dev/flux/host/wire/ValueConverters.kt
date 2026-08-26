package dev.flux.host.wire

import dev.flux.ui.FluxValue

/**
 * Converts a wire [WireValue] (Appendix D §D.5) into the adapter kit's
 * [dev.flux.ui.FluxValue] (which the shadow tree and adapters speak).
 *
 * The one representational difference is `Str`: on the wire it is an interned
 * `u32` id. When [stringLookup] is supplied (the synced frame string table) the
 * id is resolved to its interned text; otherwise it falls back to the decimal
 * id so the runtime stays internally consistent (Appendix D §D.9).
 */
public fun WireValue.toKitValue(stringLookup: (UInt) -> String? = { null }): FluxValue =
    when (this) {
        WireValue.Null -> FluxValue.Null
        is WireValue.IntVal -> FluxValue.Int(value)
        is WireValue.FloatVal -> FluxValue.Float(value)
        is WireValue.BoolVal -> FluxValue.Bool(value)
        is WireValue.StrVal -> FluxValue.Str(stringLookup(id) ?: id.toString())
        is WireValue.HandlerRefVal -> FluxValue.HandlerRef(handlerId)
        is WireValue.ListVal -> FluxValue.List(items.map { it.toKitValue(stringLookup) })
        is WireValue.RecordVal ->
            FluxValue.Record(
                fields.map { FluxValue.Field(it.index, it.value.toKitValue(stringLookup)) },
            )
    }

/**
 * Converts the adapter kit's [dev.flux.ui.FluxValue] back into the VM's
 * [dev.flux.host.vm.FluxValue] for dispatch into [dev.flux.host.vm.FluxBytecodeVM].
 *
 * When [interning] is supplied, a resolved `Str` maps back to its wire
 * `StringId` (perf task 7, P2) through the reverse index, which resolves to the
 * canonical server id for known strings. Strings the index does not know are
 * interned through [dev.flux.host.FluxExecutor.internString] before dispatch —
 * this converter never fabricates an unstable `hashCode()` id, retiring the
 * synthetic-string brittleness (4d).
 */
public fun dev.flux.ui.FluxValue.toVmValue(interning: StringInterning? = null): dev.flux.host.vm.FluxValue =
    when (this) {
        dev.flux.ui.FluxValue.Null -> dev.flux.host.vm.FluxValue.NullVal
        is dev.flux.ui.FluxValue.Int ->
            dev.flux.host.vm.FluxValue
                .IntVal(value)
        is dev.flux.ui.FluxValue.Float ->
            dev.flux.host.vm.FluxValue
                .FloatVal(value)
        is dev.flux.ui.FluxValue.Bool ->
            dev.flux.host.vm.FluxValue
                .BoolVal(value)
        is dev.flux.ui.FluxValue.Str ->
            dev.flux.host.vm.FluxValue
                .StrVal(interning?.resolve(value) ?: STRING_ID_CANONICAL_CEILING)
        is dev.flux.ui.FluxValue.HandlerRef ->
            dev.flux.host.vm.FluxValue
                .HandlerRefVal(handlerId)
        is dev.flux.ui.FluxValue.List ->
            dev.flux.host.vm.FluxValue
                .ListVal(items.map { it.toVmValue(interning) })
        is dev.flux.ui.FluxValue.Record ->
            dev.flux.host.vm.FluxValue
                .RecordVal(
                    fields.map {
                        dev.flux.host.vm.FluxValue
                            .Field(it.index, it.value.toVmValue(interning))
                    },
                )
    }
