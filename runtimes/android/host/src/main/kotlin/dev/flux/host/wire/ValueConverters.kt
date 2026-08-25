package dev.flux.host.wire

import dev.flux.ui.FluxValue

/**
 * Converts a wire [WireValue] (Appendix D §D.5) into the adapter kit's
 * [dev.flux.ui.FluxValue] (which the shadow tree and adapters speak).
 *
 * The one representational difference is `Str`: on the wire it is an interned
 * `u32` id, but the adapter kit carries the *resolved* string. The MLP host has
 * no live string table, so it maps a string id to its decimal representation —
 * exactly the proxy the Rust oracle and the VM use for `STR_LEN`/`STR_CONCAT`,
 * keeping the runtime internally consistent without inventing a table.
 */
public fun WireValue.toKitValue(): FluxValue =
    when (this) {
        WireValue.Null -> FluxValue.Null
        is WireValue.IntVal -> FluxValue.Int(value)
        is WireValue.FloatVal -> FluxValue.Float(value)
        is WireValue.BoolVal -> FluxValue.Bool(value)
        is WireValue.StrVal -> FluxValue.Str(id.toString())
        is WireValue.HandlerRefVal -> FluxValue.HandlerRef(handlerId)
        is WireValue.ListVal -> FluxValue.List(items.map { it.toKitValue() })
        is WireValue.RecordVal ->
            FluxValue.Record(
                fields.map { FluxValue.Field(it.index, it.value.toKitValue()) },
            )
    }

/**
 * Converts the adapter kit's [dev.flux.ui.FluxValue] back into the VM's
 * [dev.flux.host.vm.FluxValue] for dispatch into [dev.flux.host.vm.FluxBytecodeVM].
 *
 * When [interning] is supplied, a resolved `Str` maps back to its canonical
 * wire `StringId` (perf task 7, P2) instead of an unstable `hashCode()`.
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
            dev.flux.host.vm.FluxValue.StrVal(
                interning?.resolve(value) ?: (value.toUIntOrNull() ?: value.hashCode().toUInt()),
            )
        is dev.flux.ui.FluxValue.HandlerRef ->
            dev.flux.host.vm.FluxValue
                .HandlerRefVal(handlerId)
        is dev.flux.ui.FluxValue.List ->
            dev.flux.host.vm.FluxValue
                .ListVal(items.map { it.toVmValue(interning) })
        is dev.flux.ui.FluxValue.Record ->
            dev.flux.host.vm.FluxValue.RecordVal(
                fields.map {
                    dev.flux.host.vm.FluxValue
                        .Field(it.index, it.value.toVmValue(interning))
                },
            )
    }
