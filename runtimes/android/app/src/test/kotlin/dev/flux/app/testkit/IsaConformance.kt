package dev.flux.app.testkit

import dev.flux.app.vm.FluxBytecodeVM
import dev.flux.app.vm.FluxValue
import dev.flux.app.vm.InMemorySignals
import dev.flux.app.vm.VmErrorKind
import dev.flux.app.vm.VmResult
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DynamicTest
import java.io.File

/**
 * ISA conformance harness: loads every golden vector from
 * `/tests/isa-vectors` (read-only JSON, shared with `flux-vm-ref`) and
 * asserts the Kotlin [FluxBytecodeVM] agrees with the oracle on
 * signals/registers/errors/gas — the FLUX-007 acceptance criterion.
 *
 * The vectors are the behavioral contract (FLUX-002). The harness never edits
 * them: any divergence is reported to the orchestrator, never papered over.
 */
public object IsaConformance {
    /** Locates the frozen vector directory relative to the repo root. */
    public fun vectorDir(): File {
        // Test cwd is the module dir (runtimes/android/app); the vectors live at
        // <repo>/tests/isa-vectors. Walk up to the repo root and descend.
        var dir = File(System.getProperty("user.dir") ?: ".").canonicalFile
        repeat(6) {
            val candidate = File(dir, "tests/isa-vectors")
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile ?: return@repeat
        }
        return File("tests/isa-vectors")
    }

    /** Loads and parses every vector JSON into [Vector] records. */
    public fun loadVectors(): List<Vector> {
        val dir = vectorDir()
        assertTrue(dir.isDirectory, "isa-vectors dir not found at ${dir.absolutePath}")
        return dir
            .listFiles { f -> f.extension == "json" }!!
            .map { Vector.parse(it.readText()) }
            .sortedBy { it.name }
            .also { assertTrue(it.isNotEmpty(), "no vectors loaded from ${dir.absolutePath}") }
    }

    /** Builds a JUnit [DynamicTest] per vector that asserts oracle agreement. */
    public fun dynamicTests(): List<DynamicTest> =
        loadVectors().map { v ->
            DynamicTest.dynamicTest(v.name) { runVector(v) }
        }

    /** Runs one [Vector] through the Kotlin VM and asserts expected outcomes. */
    public fun runVector(v: Vector) {
        val bytecode = Hex.decode(v.bytecodeHex)
        val signals =
            InMemorySignals.fromSignals(
                v.initialSignals.map { (id, value) -> id to value.toFluxValue() },
            )
        val payload = v.payload?.toFluxValue() ?: FluxValue.NullVal

        when (v.expectedError) {
            null -> {
                val result = FluxBytecodeVM.run(bytecode, signals, payload)
                assertTrue(result is VmResult.Success, "${v.name}: unexpected $result")
                result as VmResult.Success
                assertEquals(v.expectedGas, result.outcome.gasUsed, "${v.name}: gas mismatch")
                for ((id, value) in v.expectedSignals) {
                    val got = signals.read(id)
                    assertEquals(value.toFluxValue(), got, "${v.name}: signal $id mismatch")
                }
                for ((name, value) in v.expectedRegisters) {
                    val idx = name.substring(1).toInt()
                    assertEquals(value.toFluxValue(), result.outcome.registers[idx], "${v.name}: register $name mismatch")
                }
            }
            else -> {
                val result = FluxBytecodeVM.run(bytecode, signals, payload)
                assertTrue(result is VmResult.Failure, "${v.name}: expected error ${v.expectedError} but succeeded")
                result as VmResult.Failure
                assertEquals(v.expectedError, result.kind, "${v.name}: error kind mismatch")
            }
        }
    }
}

/** Parsed golden ISA vector (mirrors the JSON schema in `tests/isa-vectors`). */
public data class Vector(
    val name: String,
    val bytecodeHex: String,
    val initialSignals: List<Pair<UInt, VecValue>>,
    val payload: VecValue?,
    val expectedSignals: List<Pair<UInt, VecValue>>,
    val expectedRegisters: Map<String, VecValue>,
    val expectedError: VmErrorKind?,
    val expectedGas: UInt,
) {
    public companion object {
        public fun parse(json: String): Vector {
            @Suppress("UNCHECKED_CAST")
            val root = MiniJson.parse(json) as Map<String, Any?>
            val name = root["name"] as String
            val bytecodeHex = root["bytecode_hex"] as String
            val initialSignals = (root["initial_signals"] as List<*>).map { sigFrom(it) }
            val payload = root["payload"]?.let { VecValue.parse(it) }
            val expectedSignals = (root["expected_signals"] as List<*>).map { sigFrom(it) }
            val expectedRegisters =
                (root["expected_registers"] as Map<*, *>)
                    .map { (k, v) -> (k as String) to VecValue.parse(v!!) }
                    .toMap()
            val expectedError = (root["expected_error"] as? String)?.let { errorFrom(it) }
            val expectedGas = (root["expected_gas_used"] as Number).toLong().toUInt()
            return Vector(name, bytecodeHex, initialSignals, payload, expectedSignals, expectedRegisters, expectedError, expectedGas)
        }
    }
}

private fun sigFrom(raw: Any?): Pair<UInt, VecValue> {
    @Suppress("UNCHECKED_CAST")
    val m = raw as Map<String, Any?>
    return (m["id"] as Number).toLong().toUInt() to VecValue.parse(m["value"]!!)
}

private fun errorFrom(s: String): VmErrorKind =
    when (s) {
        "GasExhausted" -> VmErrorKind.GAS_EXHAUSTED
        "MemoryExhausted" -> VmErrorKind.MEMORY_EXHAUSTED
        "IndexOutOfBounds" -> VmErrorKind.INDEX_OUT_OF_BOUNDS
        "NullDereference" -> VmErrorKind.NULL_DEREFERENCE
        "InvalidDispatch" -> VmErrorKind.INVALID_DISPATCH
        "TypeMismatch" -> VmErrorKind.TYPE_MISMATCH
        "DivByZero" -> VmErrorKind.DIV_BY_ZERO
        else -> throw IllegalArgumentException("unknown error kind: $s")
    }

/** A decoded JSON value from the vector schema. */
public data class VecValue(
    val ty: String,
    val raw: Any?,
) {
    public fun toFluxValue(): FluxValue =
        when (ty) {
            "Int" -> FluxValue.IntVal((raw as Number).toLong())
            "Float" -> FluxValue.FloatVal(parseFloat(raw))
            "Bool" ->
                FluxValue.BoolVal(
                    when (raw) {
                        is Boolean -> raw
                        is Number -> raw.toLong() != 0L
                        is String -> raw == "true" || raw.toLongOrNull()?.let { it != 0L } ?: false
                        else -> throw IllegalArgumentException("cannot coerce to Bool: $raw")
                    },
                )
            "Str" -> FluxValue.StrVal((raw as Number).toLong().toUInt())
            "Null" -> FluxValue.NullVal
            "List" -> FluxValue.ListVal((raw as List<*>).map { VecValue.parse(it!!).toFluxValue() })
            "Record" ->
                FluxValue.RecordVal(
                    (raw as List<*>).mapIndexed { i, e -> FluxValue.Field(i.toUShort(), VecValue.parse(e!!).toFluxValue()) },
                )
            else -> throw IllegalArgumentException("unknown value type tag: $ty")
        }

    public companion object {
        /**
         * Parses a value from either the wrapped `{"type":..,"value":..}` form
         * (used by `initial_signals`/`expected_signals`) or the raw tuple
         * `["Type", value]` form (used by `payload`) so the harness accepts
         * every vector layout without editing the frozen fixtures.
         */
        public fun parse(raw: Any?): VecValue =
            when (raw) {
                is Map<*, *> -> {
                    @Suppress("UNCHECKED_CAST")
                    val m = raw as Map<String, Any?>
                    VecValue(m["type"] as String, m["value"])
                }
                is List<*> -> {
                    val list = raw
                    VecValue(list[0] as String, list.getOrNull(1))
                }
                else -> throw IllegalArgumentException("value is neither wrapped nor tuple: $raw")
            }
    }
}

private fun parseFloat(raw: Any?): Double =
    when (raw) {
        is Number -> raw.toDouble()
        is String ->
            when (raw) {
                "inf" -> Double.POSITIVE_INFINITY
                "-inf" -> Double.NEGATIVE_INFINITY
                "nan" -> Double.NaN
                else -> raw.toDouble()
            }
        else -> 0.0
    }

/** Minimal hex decoder (lowercase, no spaces) for `bytecode_hex`. */
private object Hex {
    public fun decode(s: String): ByteArray {
        val clean = s.replace(" ", "")
        require(clean.length % 2 == 0) { "odd hex length" }
        return ByteArray(clean.length / 2) { i ->
            clean.substring(2 * i, 2 * i + 2).toInt(16).toByte()
        }
    }
}

/** Tiny self-contained JSON parser (objects, arrays, strings, numbers, bool, null). */
private object MiniJson {
    public fun parse(s: String): Any? = Parser(s).parseValue()

    private class Parser(
        private val s: String,
    ) {
        private var i = 0

        fun parseValue(): Any? {
            skipWs()
            return when (val c = s[i]) {
                '{' -> parseObject()
                '[' -> parseArray()
                '"' -> parseString()
                't', 'f' -> parseBool()
                'n' -> parseNull()
                else -> parseNumber()
            }
        }

        private fun parseObject(): Map<String, Any?> {
            expect('{')
            val map = LinkedHashMap<String, Any?>()
            skipWs()
            if (peek() == '}') {
                i++
                return map
            }
            while (true) {
                skipWs()
                val key = parseString()
                skipWs()
                expect(':')
                skipWs()
                map[key] = parseValue()
                skipWs()
                val c = s[i]
                if (c == ',') {
                    i++
                    continue
                }
                if (c == '}') {
                    i++
                    break
                }
                throw IllegalArgumentException("expected , or } at $i")
            }
            return map
        }

        private fun parseArray(): List<Any?> {
            expect('[')
            val list = ArrayList<Any?>()
            skipWs()
            if (peek() == ']') {
                i++
                return list
            }
            while (true) {
                list.add(parseValue())
                skipWs()
                val c = s[i]
                if (c == ',') {
                    i++
                    continue
                }
                if (c == ']') {
                    i++
                    break
                }
                throw IllegalArgumentException("expected , or ] at $i")
            }
            return list
        }

        private fun parseString(): String {
            expect('"')
            val sb = StringBuilder()
            while (i < s.length && s[i] != '"') {
                val c = s[i++]
                if (c == '\\') {
                    val e = s[i++]
                    sb.append(
                        when (e) {
                            '"' -> '"'
                            '\\' -> '\\'
                            '/' -> '/'
                            'b' -> '\b'
                            'f' -> '\u000C'
                            'n' -> '\n'
                            'r' -> '\r'
                            't' -> '\t'
                            'u' ->
                                s
                                    .substring(i, i + 4)
                                    .also { i += 4 }
                                    .toInt(16)
                                    .toChar()
                            else -> e
                        },
                    )
                } else {
                    sb.append(c)
                }
            }
            expect('"')
            return sb.toString()
        }

        private fun parseBool(): Boolean {
            if (s.startsWith("true", i)) {
                i += 4
                return true
            }
            if (s.startsWith("false", i)) {
                i += 5
                return false
            }
            throw IllegalArgumentException("bad literal at $i")
        }

        private fun parseNull(): Nothing? {
            if (s.startsWith("null", i)) {
                i += 4
            }
            return null
        }

        private fun parseNumber(): Any {
            val start = i
            while (i < s.length && s[i] in "-0123456789.eE+".toCharArray()) i++
            val text = s.substring(start, i)
            // Preserve the schema: integers stay Long, floats stay Double.
            return if (text.contains('.') || text.contains('e', true)) text.toDouble() else text.toLong()
        }

        private fun skipWs() {
            while (i < s.length && s[i] in " \t\n\r".toCharArray()) i++
        }

        private fun peek(): Char = s[i]

        private fun expect(c: Char) {
            if (s[i] != c) throw IllegalArgumentException("expected '$c' at $i, got '${s[i]}'")
            i++
        }
    }
}
