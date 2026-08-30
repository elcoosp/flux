package dev.flux.host.vm

import kotlin.math.absoluteValue
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Tri-platform parity gate for the Kotlin [FluxBytecodeVM] (FLUX-089).
 *
 * Loads every frozen golden ISA vector under `tests/isa-vectors/` — the same
 * suite the Rust reference oracle (`flux-vm-ref`) and the Swift `FluxBytecodeVM`
 * run — and asserts the native Kotlin VM produces identical signals, registers,
 * error kinds and gas usage. A host-VM divergence from the oracle is exactly the
 * surface `cargo-mutants` (a Rust-only tool) cannot catch, so this test is the
 * Kotlin half of the FLUX-089 parity-mutation gate.
 *
 * The suite is a single aggregator test that mirrors
 * `crates/flux-vm-ref/tests/conformance.rs` and
 * `runtimes/ios/FluxHost/Tests/FluxHostTests/ISAConformanceTests.swift`: every
 * vector is checked and all failures are reported together, so a partial
 * regression surfaces the full set rather than failing on the first vector.
 */
class IsaConformanceVmTest {
    /** Locates the frozen ISA vectors directory. */
    private fun vectorsDirectory(): File? {
        val env = System.getenv("FLUX_ISA_VECTORS")
        val seeds = mutableListOf<String>()
        if (env != null) seeds += env
        // Gradle runs host tests with cwd = the module dir (runtimes/android/host);
        // the vectors live at <repo>/tests/isa-vectors, three levels up.
        seeds += System.getProperty("user.dir")
        seeds += listOf(
            "tests/isa-vectors",
            "../tests/isa-vectors",
            "../../tests/isa-vectors",
            "../../../tests/isa-vectors",
            "../../../../tests/isa-vectors",
            "../../../../../tests/isa-vectors",
        )
        for (seed in seeds) {
            var dir = File(seed)
            for (step in 0..6) {
                val candidate = File(dir, "tests/isa-vectors")
                if (candidate.isDirectory) return candidate
                dir = dir.parentFile ?: break
            }
        }
        return null
    }

    @Test
    fun `kotlin vm passes every golden isa vector`() {
        val dir = vectorsDirectory()
            ?: throw AssertionError(
                "ISA vectors not found; set FLUX_ISA_VECTORS to the directory",
            )
        val urls = dir.listFiles { f -> f.extension == "json" }
            ?.sortedBy { it.name }
            ?: emptyList()
        assertFalse(urls.isEmpty(), "no vectors loaded from ${dir.path}")

        var passed = 0
        val failures = mutableListOf<String>()
        for (file in urls) {
            val vector = JSONObject(file.readText())
            val name = vector.getString("name")
            val bytecode = vector.getString("bytecode_hex").hexBytes()
            val signals =
                InMemorySignals.fromSignals(
                    vector.optJSONArray("initial_signals")?.toSignalSeeds().orEmpty()
                        .map { (id, raw) -> id to jsonToFluxValue(raw) },
                )
            val payload = vector.opt("payload")?.let { jsonToFluxValue(it) } ?: FluxValue.NullVal
            val expectedError = vector.optString("expected_error", null)

            if (expectedError != null) {
                val kind = expectedError.toVmErrorKind()
                when (val res = FluxBytecodeVM.run(bytecode, signals, payload)) {
                    is VmResult.Failure -> {
                        if (res.kind != kind) {
                            failures += "$name: expected error $expectedError got ${res.kind}"
                        }
                    }
                    is VmResult.Success ->
                        failures += "$name: expected error $expectedError but succeeded"
                }
            } else {
                val out = when (
                    val res = FluxBytecodeVM.run(bytecode, signals, payload)
                ) {
                    is VmResult.Success -> res.outcome
                    is VmResult.Failure -> {
                        failures += "$name: unexpected error ${res.kind}"
                        continue
                    }
                }
                val expectedGas = vector.optInt("expected_gas_used", -1)
                if (expectedGas >= 0 && out.gasUsed.toInt() != expectedGas) {
                    failures += "$name: gas ${out.gasUsed} != expected $expectedGas"
                }
                val finalSignals = out.signals.toMap()
                vector.optJSONArray("expected_signals")?.toSignalSeeds().orEmpty().forEach { (id, raw) ->
                    val got = finalSignals[id]
                    if (got == null) {
                        failures += "$name: signal $id missing"
                    } else if (!valueMatches(got, raw)) {
                        failures += "$name: signal $id mismatch: $got"
                    }
                }
                vector.optJSONObject("expected_registers")?.let { regs ->
                    regs.keys().forEach { key ->
                        val idx = key.removePrefix("r").toInt()
                        if (!valueMatches(out.registers[idx], regs.get(key))) {
                            failures += "$name: register $key mismatch: ${out.registers[idx]}"
                        }
                    }
                }
            }
            passed += 1
        }

        assertTrue(
            failures.isEmpty(),
            "${failures.size} of ${urls.size} vectors FAILED:\n${failures.joinToString("\n")}",
        )
        println("kotlin conformance: $passed/${urls.size} vectors passed")
    }

    // MARK: - recursive vector value conversion

    /**
     * Converts a vector value element into a [FluxValue], mirroring the Rust
     * oracle's `to_value`, which operates on a raw `serde_json::Value`. Two
     * encodings are accepted so the suite agrees with the frozen vectors exactly:
     * - object form `{"type": "...", "value": ...}` (used by every expected_* and
     *   most payloads);
     * - array form `["Type", value]` (used by `reg_r0_payload`'s `payload`, where
     *   `value` for `Record`/`List` is itself an array of `[type, value]` entries).
     */
    private fun jsonToFluxValue(any: Any?): FluxValue = when (any) {
        null -> FluxValue.NullVal
        is JSONObject -> buildFromType(any.optString("type", "Null"), any.opt("value"))
        is JSONArray -> {
            // Array form: [typeString, value?].
            val type = (any.opt(0) as? String) ?: "Null"
            val raw = if (any.length() > 1) any.opt(1) else null
            buildFromType(type, raw)
        }
        // A bare scalar (should not occur in well-formed vectors) — treat as Null.
        else -> FluxValue.NullVal
    }

    private fun buildFromType(type: String, raw: Any?): FluxValue = when (type) {
        "Int" -> FluxValue.IntVal((raw as? Number)?.toLong() ?: 0L)
        "Float" -> FluxValue.FloatVal(parseDouble(raw))
        "Bool" -> FluxValue.BoolVal((raw as? Boolean) ?: ((raw as? Number)?.toLong() != 0L))
        "Str" -> FluxValue.StrVal((raw as? Number)?.toLong()?.toUInt() ?: 0u)
        "Null" -> FluxValue.NullVal
        "List" -> FluxValue.ListVal(
            (raw as? JSONArray)?.let { arr ->
                (0 until arr.length()).map { i -> jsonToFluxValue(arr.get(i)) }
            }.orEmpty(),
        )
        "Record" -> FluxValue.RecordVal(
            (raw as? JSONArray)?.let { arr ->
                (0 until arr.length()).map { i -> FluxValue.Field(i.toUShort(), jsonToFluxValue(arr.get(i))) }
            }.orEmpty(),
        )
        else -> FluxValue.NullVal
    }

    private fun JSONArray?.toSignalSeeds(): List<Pair<UInt, Any?>> {
        if (this == null) return emptyList()
        return (0 until length()).map { i ->
            val obj = getJSONObject(i)
            obj.getLong("id").toUInt() to obj.opt("value")
        }
    }

    /** Parses a float, including the `inf`/`-inf`/`nan` string forms the vectors use. */
    private fun parseDouble(raw: Any?): Double = when (raw) {
        is Number -> raw.toDouble()
        is String -> when (raw) {
            "inf" -> Double.POSITIVE_INFINITY
            "-inf" -> Double.NEGATIVE_INFINITY
            "nan" -> Double.NaN
            else -> raw.toDoubleOrNull() ?: 0.0
        }
        else -> 0.0
    }

    /** Approximate float comparison mirroring the Rust oracle's `approx_eq`. */
    private fun valueMatches(actual: FluxValue, expectedRaw: Any?): Boolean {
        val exp = jsonToFluxValue(expectedRaw)
        return when {
            actual is FluxValue.FloatVal && exp is FluxValue.FloatVal -> approxEq(actual.value, exp.value)
            else -> actual == exp
        }
    }

    private fun approxEq(a: Double, b: Double): Boolean {
        if (a.isNaN() && b.isNaN()) return true
        if (a.isInfinite() || b.isInfinite()) return a == b
        return (a - b).absoluteValue < 1e-9
    }

    private fun String.toVmErrorKind(): VmErrorKind = when (this) {
        "GasExhausted" -> VmErrorKind.GAS_EXHAUSTED
        "MemoryExhausted" -> VmErrorKind.MEMORY_EXHAUSTED
        "IndexOutOfBounds" -> VmErrorKind.INDEX_OUT_OF_BOUNDS
        "NullDereference" -> VmErrorKind.NULL_DEREFERENCE
        "InvalidDispatch" -> VmErrorKind.INVALID_DISPATCH
        "TypeMismatch" -> VmErrorKind.TYPE_MISMATCH
        "DivByZero" -> VmErrorKind.DIV_BY_ZERO
        else -> error("unknown expected_error kind: $this")
    }

    private fun String.hexBytes(): ByteArray {
        val clean = filter { it.isLetterOrDigit() }
        require(clean.length % 2 == 0) { "odd-length hex in bytecode_hex" }
        return clean.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
}
