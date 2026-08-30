package dev.flux.host.vm

import dev.flux.host.vm.FluxValue.BoolVal
import dev.flux.host.vm.FluxValue.FloatVal
import dev.flux.host.vm.FluxValue.HandlerRefVal
import dev.flux.host.vm.FluxValue.IntVal
import dev.flux.host.vm.FluxValue.ListVal
import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import org.msgpack.core.MessagePack
import org.msgpack.core.MessagePacker
import org.msgpack.core.MessageUnpacker
import java.io.File
import java.io.IOException

/**
 * A persistence backend for stateful capabilities (e.g. `Storage`), injected
 * into [CapabilityStore] so tests can use an in-memory store while dev/release
 * builds persist to disk (Appendix E §E.1, ADR-0045).
 *
 * The backend is the seam between the capability registry (pure logic) and the
 * platform's durable storage. The MLP dev/test path registers an
 * [InMemoryStorageBackend]; the app shell registers a [FileStorageBackend] so
 * `Storage.set`/`get`/`delete` survive process restarts. Values are encoded as
 * MessagePack (Appendix D §D.5 shape) so a JVM-test has no Android-framework
 * dependency.
 */
public interface StorageBackend {
    /** Records [value] for [key]; `null` clears it. */
    public fun put(
        key: UInt,
        value: FluxValue?,
    )

    /** Reads the value for [key], or `null` if absent. */
    public fun get(key: UInt): FluxValue?

    /** Enumerates every stored entry (FLUX-047 `Persist.query`). */
    public fun entries(): Map<UInt, FluxValue>
}

/** In-memory backend: the MLP dev/test default (values live for the store's lifetime). */
public class InMemoryStorageBackend : StorageBackend {
    private val storage = LinkedHashMap<UInt, FluxValue>()

    override fun put(
        key: UInt,
        value: FluxValue?,
    ) {
        if (value == null) storage.remove(key) else storage[key] = value
    }

    override fun get(key: UInt): FluxValue? = storage[key]

    override fun entries(): Map<UInt, FluxValue> = LinkedHashMap(storage)
}

/**
 * File-backed backend: real persistence for dev/release builds.
 *
 * Each `Storage` value is encoded as MessagePack (Appendix D §D.5) under a
 * namespaced filename `flux.storage.<keyId>.mp` inside [dir] (a JVM-test passes
 * a per-test temp dir; the app shell passes a `BuildConfig`-gated path under
 * `context.filesDir`). Dropping the registry and recreating one over the same
 * directory proves the value came from disk, not an in-memory cache.
 *
 * @property dir the directory holding persisted entries.
 */
public class FileStorageBackend(
    private val dir: File,
) : StorageBackend {
    init {
        dir.mkdirs()
    }

    private fun file(key: UInt): File = File(dir, "flux.storage.$key.mp")

    /** A per-`put` temp file under [dir]; `renameTo` makes the swap atomic. */
    private fun tempFile(key: UInt): File = File(dir, "flux.storage.$key.mp.tmp-${System.nanoTime()}")

    /**
     * Reads and decodes [f]; on any decode failure deletes the corrupt file and
     * returns `null` (mirrors the iOS `try?`-to-`nil` contract so a torn entry is
     * treated as absent rather than crashing capability dispatch).
     */
    private fun decodeOrNull(f: File): FluxValue? {
        if (!f.exists()) return null
        return try {
            MessagePack.newDefaultUnpacker(f.inputStream()).use { unpacker -> unpacker.fluxUnpack() }
        } catch (_: Exception) {
            f.delete()
            null
        }
    }

    override fun put(
        key: UInt,
        value: FluxValue?,
    ) {
        val f = file(key)
        if (value == null) {
            f.delete()
            return
        }
        // Write to a temp file, then atomically rename it over the destination.
        // A crash mid-write (OOM kill, `adb reboot`, low-battery kill) leaves at
        // most a stray `.tmp-*` file, never a truncated `.mp` the next `get`
        // would try to decode (FLUX-080).
        val tmp = tempFile(key)
        try {
            MessagePack.newDefaultPacker(tmp.outputStream()).use { packer ->
                packer.fluxPack(value)
            }
        } catch (e: IOException) {
            tmp.delete()
            throw e
        }
        val renamed = tmp.renameTo(f)
        if (!renamed) {
            tmp.delete()
            throw IOException("failed to atomically publish storage entry for key $key")
        }
    }

    override fun get(key: UInt): FluxValue? = decodeOrNull(file(key))

    override fun entries(): Map<UInt, FluxValue> {
        val result = LinkedHashMap<UInt, FluxValue>()
        dir.listFiles { _, name -> name.startsWith("flux.storage.") && name.endsWith(".mp") }?.forEach { f ->
            val key = f.name.removePrefix("flux.storage.").removeSuffix(".mp").toUIntOrNull() ?: return@forEach
            val value = decodeOrNull(f) ?: return@forEach
            result[key] = value
        }
        return result
    }
}

/**
 * Encodes a `FluxValue` as MessagePack: every variant is a 2-element array
 * `[tag, payload]`, where `tag` is the Appendix D §D.5 wire type tag. Nested
 * `list`/`record` values are themselves encoded the same way.
 */
private fun MessagePacker.fluxPack(value: FluxValue) {
    when (value) {
        is NullVal -> packArrayHeader(2).packLong(0)
        is IntVal -> packArrayHeader(2).packLong(1).packLong(value.value)
        is FloatVal -> packArrayHeader(2).packLong(2).packDouble(value.value)
        is BoolVal -> packArrayHeader(2).packLong(3).packBoolean(value.value)
        is StrVal -> packArrayHeader(2).packLong(4).packLong(value.id.toLong())
        is HandlerRefVal -> packArrayHeader(2).packLong(5).packLong(value.handlerId.toLong())
        is ListVal -> {
            packArrayHeader(2)
            packLong(6)
            packArrayHeader(value.items.size)
            for (item in value.items) fluxPack(item)
        }
        is RecordVal -> {
            packArrayHeader(2)
            packLong(7)
            packArrayHeader(value.fields.size)
            for (field in value.fields) {
                packArrayHeader(2)
                packLong(field.index.toLong())
                fluxPack(field.value)
            }
        }
    }
}

/**
 * Decodes a `FluxValue` from MessagePack (inverse of [fluxPack]).
 *
 * Every encoded value is a 2-element array `[tag, payload]`; the payload shape
 * depends on `tag` (Appendix D §D.5).
 */
private fun MessageUnpacker.fluxUnpack(): FluxValue {
    val header = unpackArrayHeader()
    if (header != 2) throw VmError(VmErrorKind.TYPE_MISMATCH, 0u)
    return when (val tag = unpackLong().toInt()) {
        0 -> NullVal
        1 -> IntVal(unpackLong())
        2 -> FloatVal(unpackDouble())
        3 -> BoolVal(unpackBoolean())
        4 -> StrVal(unpackLong().toUInt())
        5 -> HandlerRefVal(unpackLong().toUInt())
        6 -> {
            val n = unpackArrayHeader()
            val items = ArrayList<FluxValue>(n)
            repeat(n) { items += fluxUnpack() }
            ListVal(items)
        }
        7 -> {
            val n = unpackArrayHeader()
            val fields = ArrayList<FluxValue.Field>(n)
            repeat(n) {
                val fl = unpackArrayHeader()
                if (fl != 2) throw VmError(VmErrorKind.TYPE_MISMATCH, 0u)
                val idx = unpackLong().toUShort()
                fields += FluxValue.Field(idx, fluxUnpack())
            }
            RecordVal(fields)
        }
        else -> throw VmError(VmErrorKind.TYPE_MISMATCH, 0u)
    }
}
