package dev.flux.host.shadow

import dev.flux.host.AdapterRegistry
import dev.flux.host.testkit.IsaConformance
import org.json.JSONObject
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * Asserts the Kotlin ForEach id-derivation matches the frozen vectors in
 * `tests/isa-vectors/foreach_ids.json` (shared with the Swift host and the
 * reference Rust impl — FLUX-007 behavioral contract).
 *
 * Also enforces the Phase 3 collision guard: every derived id must carry its
 * marker bits so it can never collide with a real server node id (< 0x8000_0000).
 */
class ForEachIdDerivationTest {
    private val tree: ShadowTree = ShadowTree(AdapterRegistry.fromStringTable(emptyList()))

    /** Parses the frozen vector file and returns the list of vector objects. */
    private fun loadVectors(): List<JSONObject> {
        val dir = IsaConformance.vectorDir()
        val file = java.io.File(dir, "foreach_ids.json")
        val json = JSONObject(file.readText())
        val arr = json.getJSONArray("vectors")
        return List(arr.length()) { arr.getJSONObject(it) }
    }

    @Test
    fun `row id matches frozen vector for index-based seed`() {
        for (v in loadVectors()) {
            val inp = v.getJSONObject("input")
            val foreachId = inp.getLong("foreach_id").toUInt()
            val seed =
                if (inp.has("splice_key")) {
                    inp.getLong("splice_key").toULong()
                } else {
                    inp.getLong("row_index").toULong()
                }
            val expected = v.getString("row_id").removePrefix("0x").toUInt(16)

            val got = tree.deriveForEachRowId(foreachId, seed)

            assertEquals(expected, got, "row_id mismatch for input=$v")
        }
    }

    @Test
    fun `child id matches frozen vector for orig id`() {
        for (v in loadVectors()) {
            val inp = v.getJSONObject("input")
            val foreachId = inp.getLong("foreach_id").toUInt()
            val seed =
                if (inp.has("splice_key")) {
                    inp.getLong("splice_key").toULong()
                } else {
                    inp.getLong("row_index").toULong()
                }
            val origId = inp.getLong("orig_id").toUInt()
            val expected = v.getString("child_id").removePrefix("0x").toUInt(16)

            val rowId = tree.deriveForEachRowId(foreachId, seed)
            val got = tree.deriveForEachChildId(rowId, origId)

            assertEquals(expected, got, "child_id mismatch for input=$v")
        }
    }

    @Test
    fun `key child id matches frozen vector for splice key`() {
        for (v in loadVectors()) {
            val inp = v.getJSONObject("input")
            if (!inp.has("splice_key")) continue
            val foreachId = inp.getLong("foreach_id").toUInt()
            val key = inp.getLong("splice_key").toULong()
            val origId = inp.getLong("orig_id").toUInt()
            val expected = v.getString("key_child_id").removePrefix("0x").toUInt(16)

            val rowId = tree.deriveForEachRowId(foreachId, key)
            val got = tree.deriveForEachKeyChild(rowId, key)

            assertEquals(expected, got, "key_child_id mismatch for input=$v")
        }
    }

    @Test
    fun `collision guard - all derived ids carry marker bits`() {
        // Test over the frozen vectors AND additional random-ish inputs.
        val inputs =
            listOf(
                1u to 0uL,
                1u to 1uL,
                1u to 42uL,
                20u to 1uL,
                20u to 0uL,
                0xFFFFFFFFu to 0xFFFFFFFFuL,
            )
        for ((fid, seed) in inputs) {
            val rowId = tree.deriveForEachRowId(fid, seed)
            assert(rowId and 0x8000_0000u != 0u) { "row id missing marker: 0x${rowId.toString(16)}" }
        }
        for ((fid, seed) in inputs) {
            val rowId = tree.deriveForEachRowId(fid, seed)
            val childId = tree.deriveForEachChildId(rowId, seed.toUInt())
            assert(childId and 0xC000_0000u == 0xC000_0000u) {
                "child id missing marker: 0x${childId.toString(16)}"
            }
            val keyChildId = tree.deriveForEachKeyChild(rowId, seed)
            assert(keyChildId and 0xC000_0000u == 0xC000_0000u) {
                "key-child id missing marker: 0x${keyChildId.toString(16)}"
            }
        }
    }

    @Test
    fun `splice key changes row id (D1 divergence)`() {
        val foreachId = 20u
        val indexed = tree.deriveForEachRowId(foreachId, 1uL)
        val keyed = tree.deriveForEachRowId(foreachId, 0x0100_0000_0000_0001uL)
        assert(indexed != keyed) { "splice key must change the row id" }
    }
}
