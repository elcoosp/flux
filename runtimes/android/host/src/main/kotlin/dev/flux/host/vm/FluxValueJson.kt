package dev.flux.host.vm

import dev.flux.host.vm.FluxValue.BoolVal
import dev.flux.host.vm.FluxValue.Field
import dev.flux.host.vm.FluxValue.FloatVal
import dev.flux.host.vm.FluxValue.IntVal
import dev.flux.host.vm.FluxValue.ListVal
import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONException

/**
 * JSON → [FluxValue] parser (FLUX-047 `Http.getJson` / `postJson` response
 * parsing), built on the `org.json` library (runs on the plain JVM, so the
 * `:host` suite stays Android-free).
 *
 * Mapping (mirrors the iOS `JSONSerialization`-backed parser):
 * - JSON object  → [RecordVal] keyed by the field's string index (same shape a
 *   struct/record lowers to on the wire), so a `.flux` handler can read fields
 *   by `propIndex` after the response is bound.
 * - JSON array   → [ListVal].
 * - string       → [StrVal] carrying the *interned* id (use the executor's
 *   `StringResolver.intern` so the id matches the live wire string table). A
 *   bare fallback FNV-1a interner is supplied for test/dev paths without a
 *   resolver.
 * - number       → [IntVal] when integral, otherwise [FloatVal].
 * - boolean      → [BoolVal].
 * - null         → [NullVal].
 *
 * A parse failure yields [NullVal] — a network fault must never crash the host;
 * the `.flux` handler can branch on the null.
 */
public object FluxValueJson {
    /** FNV-1a-32 interim interner for dev/test paths without a live resolver. */
    private fun fnv1a(text: String): UInt {
        var hash: UInt = 0x811c9dc5u
        for (b in text.encodeToByteArray()) {
            hash = (hash xor b.toUInt()) * 0x01000193u
        }
        return hash and 0x0FFFFFFFu
    }

    /**
     * Parses [text] as JSON, returning a [FluxValue]; [NullVal] on any error.
     * - [intern] maps a string to an interned [UInt] id for [StrVal] leaves;
     *   default is a local FNV-1a interner (sufficient for equality checks in
     *   tests and dev rendering where the id need not match the wire table).
     */
    public fun parse(text: String, intern: (String) -> UInt = ::fnv1a): FluxValue =
        try {
            parseValue(JSONObject(text), intern)
        } catch (e: JSONException) {
            // `text` may be a bare array/value; retry as an array, then scalar.
            try {
                parseValue(JSONArray(text), intern)
            } catch (_: JSONException) {
                NullVal
            }
        }

    private fun parseValue(any: Any?, intern: (String) -> UInt): FluxValue =
        when (any) {
            null -> NullVal
            is JSONObject -> {
                val fields = mutableListOf<Field>()
                val keys = any.keys()
                var index: UShort = 0u
                while (keys.hasNext()) {
                    val key = keys.next()
                    fields.add(Field(index, StrVal(intern(key))))
                    index = (index + 1u).toUShort()
                }
                RecordVal(fields)
            }
            is JSONArray -> {
                val items = ArrayList<FluxValue>(any.length())
                for (i in 0 until any.length()) items.add(parseValue(any.get(i), intern))
                ListVal(items)
            }
            is String -> StrVal(intern(any))
            is Boolean -> BoolVal(any)
            is Int -> IntVal(any.toLong())
            is Long -> IntVal(any)
            is Double -> FloatVal(any)
            is Float -> FloatVal(any.toDouble())
            else -> NullVal
        }
}
