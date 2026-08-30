package dev.flux.host.vm

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.io.File

/**
 * FLUX-080: `FileStorageBackend` must write atomically (temp file + `renameTo`)
 * and treat a corrupt/torn entry as absent (delete + return `null`) rather than
 * letting a decode exception propagate into capability dispatch — matching the
 * iOS `try?`-to-`nil` contract in `StorageBackend.swift`.
 *
 * Pure JVM (no Android framework); each test gets its own temp [dir].
 */
class StorageBackendTest {
    @Test
    fun `torn file in get is treated as absent and the corrupt file is deleted`(
        @TempDir dir: File,
    ) {
        val backend = FileStorageBackend(dir)
        val key = 42u
        // Simulate a crash mid-write: a truncated MessagePack blob where the
        // decoder expects a 2-element array header but the bytes stop early.
        File(dir, "flux.storage.$key.mp").writeBytes(byteArrayOf(0x92.toByte()))
        assertTrue(File(dir, "flux.storage.$key.mp").exists(), "precondition: corrupt file present")

        assertNull(backend.get(key), "corrupt entry must read as absent, not throw")
        assertFalse(File(dir, "flux.storage.$key.mp").exists(), "corrupt entry must be deleted on read")
    }

    @Test
    fun `entries skips and deletes a corrupt entry instead of aborting enumeration`(
        @TempDir dir: File,
    ) {
        val backend = FileStorageBackend(dir)
        val good = 1u
        val bad = 2u
        backend.put(good, FluxValue.IntVal(7))
        // Drop a second, corrupt `.mp` file directly on disk.
        File(dir, "flux.storage.$bad.mp").writeBytes(byteArrayOf(0x92.toByte()))

        val result = backend.entries()

        assertEquals(FluxValue.IntVal(7), result[good], "the good entry must still be enumerated")
        assertFalse(result.containsKey(bad), "the corrupt entry must not appear")
        assertFalse(File(dir, "flux.storage.$bad.mp").exists(), "the corrupt entry must be deleted")
    }

    @Test
    fun `successful put leaves no temp file behind`(
        @TempDir dir: File,
    ) {
        val backend = FileStorageBackend(dir)
        backend.put(10u, FluxValue.IntVal(123))

        val temps = dir.listFiles { _, name -> name.startsWith("flux.storage.") && name.contains(".tmp-") }
        assertTrue((temps?.size ?: 0) == 0, "no temp file may survive a successful put")
    }

    @Test
    fun `round-trip survives process restart parity with a fresh backend over the same dir`(
        @TempDir dir: File,
    ) {
        val writer = FileStorageBackend(dir)
        writer.put(99u, FluxValue.StrVal(555u))

        // A brand-new backend over the same directory proves the value came from
        // disk, not an in-memory cache (mirrors the iOS suite-scoped round-trip).
        val reader = FileStorageBackend(dir)
        assertEquals(FluxValue.StrVal(555u), reader.get(99u))
        assertEquals(FluxValue.StrVal(555u), reader.entries()[99u])
    }
}
