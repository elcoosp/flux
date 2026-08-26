package dev.flux.host.vm.debug

import dev.flux.host.vm.FluxValue
import java.io.ByteArrayOutputStream

/**
 * DevTools bidirectional debug telemetry (spec §3, §4).
 *
 * Host-side mirror of `flux_ir_serde::TelemetryEvent` / `DebugCommand` (Appendix
 * D §D.12, ADR-0039). The byte layout produced by [TelemetryEvent.toFrameBytes]
 * is bit-identical to `crates/flux-ir-serde/src/telemetry.rs` so the Rust dev
 * server decodes host frames without translation.
 *
 * Every call site that emits is guarded by `BuildConfig.DEBUG` (spec §3 Key
 * Principle 1: zero release impact).
 */

/** A debug telemetry event emitted by the host runtime (raw, host → server). */
public sealed interface TelemetryEvent {
    /** Emitted after each VM instruction executes. */
    public data class VmStep(
        val bytecodeOffset: UInt,
        val opcode: UByte,
        val registers: List<FluxValue>,
        val gasRemaining: UInt,
    ) : TelemetryEvent

    /** Emitted when a signal's value changes. */
    public data class SignalWrite(
        val signalId: UInt,
        val oldValue: FluxValue,
        val newValue: FluxValue,
        val triggeredEffectIds: List<UInt>,
    ) : TelemetryEvent

    /** Emitted when the reconciler mutates a native view. */
    public data class ViewMutation(
        val nodeId: UInt,
        val nativeViewId: ULong,
        val mutationKind: UByte,
        val frame: Rect?,
    ) : TelemetryEvent

    /** Emitted when a handler starts or finishes. */
    public data class HandlerInvocation(
        val handlerId: UInt,
        val isStart: Boolean,
        val gasUsed: UInt?,
    ) : TelemetryEvent
}

/** A native view layout rectangle (device points). */
public data class Rect(
    val x: Double,
    val y: Double,
    val width: Double,
    val height: Double,
)

/** A control command sent DevTools → host (Appendix D §D.12 §2.3). */
public sealed interface DebugCommand {
    public data object Pause : DebugCommand

    public data object Resume : DebugCommand

    public data object Step : DebugCommand

    public data class SetBreakpoint(
        val bytecodeOffset: UInt,
    ) : DebugCommand

    public data class ClearBreakpoint(
        val bytecodeOffset: UInt,
    ) : DebugCommand

    public data object RequestSnapshot : DebugCommand
}

/** The `MAGIC` header for every Flux wire frame (Appendix D §D.1). */
internal const val MAGIC: UInt = 0x465C5558u

internal const val FRAME_TELEMETRY: UByte = 0x10u
private const val FRAME_DEBUG_COMMAND: UByte = 0x11u

/** Encodes a [FluxValue] into [out] per Appendix D §D.5. */
private fun encodeValue(
    value: FluxValue,
    out: ByteArrayOutputStream,
) {
    when (value) {
        FluxValue.NullVal -> out.write(0x00)
        is FluxValue.IntVal -> {
            out.write(0x01)
            out.writeLongLE(value.value)
        }
        is FluxValue.FloatVal -> {
            out.write(0x02)
            out.writeLongLE(value.value.toBits())
        }
        is FluxValue.BoolVal -> {
            out.write(0x03)
            out.write(if (value.value) 1 else 0)
        }
        is FluxValue.StrVal -> {
            out.write(0x04)
            out.writeUIntLE(value.id)
        }
        is FluxValue.HandlerRefVal -> {
            out.write(0x05)
            out.writeUIntLE(value.handlerId)
        }
        is FluxValue.ListVal -> {
            out.write(0x06)
            out.writeUShortLE(value.items.size.toUShort())
            for (item in value.items) encodeValue(item, out)
        }
        is FluxValue.RecordVal -> {
            out.write(0x07)
            out.writeUShortLE(value.fields.size.toUShort())
            for (field in value.fields) {
                out.writeUShortLE(field.index)
                encodeValue(field.value, out)
            }
        }
    }
}

private fun writeUIntLE0(
    out: ByteArrayOutputStream,
    v: UInt,
) = out.write(v.toInt())

private fun writeUShortLE0(
    out: ByteArrayOutputStream,
    v: UShort,
) {
    out.write((v.toInt() and 0xFF))
    out.write((v.toInt() ushr 8) and 0xFF)
}

private fun writeLongLE0(
    out: ByteArrayOutputStream,
    v: Long,
) {
    var x = v
    repeat(8) {
        out.write((x.toInt() and 0xFF))
        x = x shr 8
    }
}

private fun ByteArrayOutputStream.writeUIntLE(v: UInt) = writeUIntLE0(this, v)

private fun ByteArrayOutputStream.writeUShortLE(v: UShort) = writeUShortLE0(this, v)

private fun ByteArrayOutputStream.writeLongLE(v: Long) = writeLongLE0(this, v)

/** Encodes this event as a length-prefixed union body (no frame header). */
internal fun TelemetryEvent.encodeBody(out: ByteArrayOutputStream) {
    val body = ByteArrayOutputStream()
    when (this) {
        is TelemetryEvent.VmStep -> {
            body.write(0x01)
            body.writeUIntLE(bytecodeOffset)
            body.write(opcode.toInt())
            for (reg in registers.take(16)) encodeValue(reg, body)
            body.writeUIntLE(gasRemaining)
        }
        is TelemetryEvent.SignalWrite -> {
            body.write(0x02)
            body.writeUIntLE(signalId)
            encodeValue(oldValue, body)
            encodeValue(newValue, body)
            body.writeUShortLE(triggeredEffectIds.size.toUShort())
            for (effect in triggeredEffectIds) body.writeUIntLE(effect)
        }
        is TelemetryEvent.ViewMutation -> {
            body.write(0x03)
            body.writeUIntLE(nodeId)
            writeULongLE(body, nativeViewId)
            body.write(mutationKind.toInt())
            if (frame != null) {
                body.write(0x01)
                writeDoubleBitsLE(body, frame.x)
                writeDoubleBitsLE(body, frame.y)
                writeDoubleBitsLE(body, frame.width)
                writeDoubleBitsLE(body, frame.height)
            } else {
                body.write(0x00)
            }
        }
        is TelemetryEvent.HandlerInvocation -> {
            body.write(0x04)
            body.writeUIntLE(handlerId)
            body.write(if (isStart) 1 else 0)
            if (gasUsed != null) {
                body.write(0x01)
                body.writeUIntLE(gasUsed)
            } else {
                body.write(0x00)
            }
        }
    }
    // Length-prefix (u32 LE) the body, back-patched at the front.
    val bytes = body.toByteArray()
    writeUIntLE0(out, bytes.size.toUInt())
    out.write(bytes)
}

private fun writeULongLE(
    out: ByteArrayOutputStream,
    v: ULong,
) {
    var x = v
    repeat(8) {
        out.write((x.toInt() and 0xFF))
        x = x shr 8
    }
}

private fun writeDoubleBitsLE(
    out: ByteArrayOutputStream,
    v: Double,
) = writeLongLE0(out, v.toBits())

/** Encodes this event into a full `Telemetry` frame (Appendix D §D.12). */
public fun TelemetryEvent.toFrameBytes(version: UByte = 0x01u): ByteArray {
    val out = ByteArrayOutputStream()
    out.writeUIntLE(MAGIC)
    out.write(version.toInt())
    out.write(FRAME_TELEMETRY.toInt())
    out.writeUShortLE(1u)
    this.encodeBody(out)
    return out.toByteArray()
}

/** Encodes this command into a full `DebugCommand` frame (kind `0x11`). */
public fun DebugCommand.toFrameBytes(
    commandId: UInt,
    version: UByte = 0x01u,
): ByteArray {
    val payload = ByteArrayOutputStream()
    when (this) {
        DebugCommand.Pause -> payload.write(0x01)
        DebugCommand.Resume -> payload.write(0x02)
        DebugCommand.Step -> payload.write(0x03)
        is DebugCommand.SetBreakpoint -> {
            payload.write(0x04)
            payload.writeUIntLE(bytecodeOffset)
        }
        is DebugCommand.ClearBreakpoint -> {
            payload.write(0x05)
            payload.writeUIntLE(bytecodeOffset)
        }
        DebugCommand.RequestSnapshot -> payload.write(0x06)
    }
    val out = ByteArrayOutputStream()
    out.writeUIntLE(MAGIC)
    out.write(version.toInt())
    out.write(FRAME_DEBUG_COMMAND.toInt())
    out.writeUIntLE(commandId)
    out.writeUShortLE(payload.size().toUShort())
    out.write(payload.toByteArray())
    return out.toByteArray()
}
