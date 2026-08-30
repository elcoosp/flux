package dev.flux.host.vm

import dev.flux.host.AsyncResolver
import dev.flux.host.vm.FluxValue.BoolVal
import dev.flux.host.vm.FluxValue.Field
import dev.flux.host.vm.FluxValue.IntVal
import dev.flux.host.vm.FluxValue.ListVal
import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import dev.flux.host.vm.VmErrorKind.TYPE_MISMATCH
import dev.flux.host.vm.debug.CAPABILITY_HTTP
import dev.flux.host.vm.debug.TelemetryBridge
import dev.flux.host.vm.debug.TelemetryEvent

/** Reads the first `StrVal` id from a record argument, or null. */
private fun firstStrId(args: FluxValue): UInt? =
    (args as? RecordVal)?.fields?.firstOrNull()?.value?.let { if (it is StrVal) it.id else null }

/**
 * FLUX-047 (the remaining host bodies): `Http` (cap 14) and `Persist` (cap 15).
 *
 * `Persist` (15) is **synchronous** — a thin queryable wrapper over the same
 * injected [StorageBackend] the `Storage` capability already uses, plus an
 * enumeration so `query` can return every stored record. Each impl returns the
 * result-cell signal id (ADR-0045) and the value is written into that cell.
 *
 * `Http` (14) is **asynchronous** (network). Its impl cannot resolve the URL /
 * body interned ids to text (it runs without a string table), so it allocates a
 * `Pending` result cell, stashes the request in [pending], and returns the cell
 * id immediately. The executor's [dev.flux.host.AsyncResolver] — which owns the
 * live [StringResolver] — later reads the request, resolves the ids to text,
 * performs the call through [transport], and settles the cell. This closes the
 * LANE-C wiring gap (the resolver previously only saw the cell id, never the
 * cap/method/args).
 *
 * These entries are added to the registry via [httpPersistEntries]; the
 * production executor wires [pending] + [transport] into [makeHttpResolver] so a
 * `fetch`/`getJson`/`postJson` cell resolves through the real network.
 *
 * @property pending the shared pending-request store keyed by result-cell id.
 * @property transport the network transport; defaults to a no-op that throws,
 *   so a registry built without a real transport fails loudly if an Http cell
 *   is ever resolved (the production app shell must inject [HttpOkHttpTransport]).
 * @property persistBackend the backend `Persist` reads/writes; defaults to an
 *   in-memory store (dev/test).
 */
public fun httpPersistEntries(
    pending: HttpRequestStore = HttpRequestStore(),
    transport: HttpTransport = object : HttpTransport {
        override fun request(method: String, url: String, body: String?): String =
            throw IllegalStateException("Http transport not configured; inject HttpOkHttpTransport")
    },
    persistBackend: StorageBackend = InMemoryStorageBackend(),
): List<Pair<CapabilityKey, CapabilityImpl>> {
    val requestStore = pending
    val requestTransport = transport
    val persist = persistBackend
    val entries = mutableListOf<Pair<CapabilityKey, CapabilityImpl>>()

    // MARK: Persist (cap 15) — synchronous structured, queryable persistence.
    // Persist.put(key, value) (15,1): store the value and return its cell id.
    entries += CapabilityKey(15u, 1u.toUShort()) to CapabilityImpl { args, signals ->
        val keyId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        val value = (args as? RecordVal)?.fields?.getOrNull(1)?.value ?: throw VmError(TYPE_MISMATCH, 0u)
        // Persist reuses the Storage store (same OS .storage gate, FLUX-047).
        persist.put(keyId, value)
        val id = signals.allocateCell()
        signals.write(id, value)
        id
    }
    // Persist.get(key) (15,2): read the stored value (default null) via its cell.
    entries += CapabilityKey(15u, 2u.toUShort()) to CapabilityImpl { args, signals ->
        val keyId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        val id = signals.allocateCell()
        signals.write(id, persist.get(keyId) ?: NullVal)
        id
    }
    // Persist.query(where) (15,3): return every stored record as a FluxValue list.
    // The `where` clause is a future filter (LANE-C: stored as raw values, so a
    // full enumeration is the correct first implementation); we return all entries.
    entries += CapabilityKey(15u, 3u.toUShort()) to CapabilityImpl { _args, signals ->
        val items = persist.entries().map { entry ->
            RecordVal(
                listOf(
                    Field(0u.toUShort(), StrVal(entry.key)),
                    Field(1u.toUShort(), entry.value),
                ),
            )
        }
        val id = signals.allocateCell()
        signals.write(id, ListVal(items))
        id
    }
    // Persist.delete(key) (15,4): clear the stored value.
    entries += CapabilityKey(15u, 4u.toUShort()) to CapabilityImpl { args, signals ->
        val keyId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        persist.put(keyId, null)
        val id = signals.allocateCell()
        signals.write(id, BoolVal(true))
        id
    }

    // MARK: Http (cap 14) — asynchronous network requests (ADR-0045).
    // Http.fetch(url, options) (14,1): allocate a Pending cell, stash the GET
    // request, return the cell id. The executor's resolver resolves it.
    entries += CapabilityKey(14u, 1u.toUShort()) to CapabilityImpl { args, signals ->
        val urlId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        val id = signals.allocateCell()
        signals.markPending(id)
        requestStore.put(id, HttpRequest(method = "GET", urlId = urlId, parseJson = false))
        id
    }
    // Http.getJson(url) (14,2): same shape, but the response is parsed to JSON.
    entries += CapabilityKey(14u, 2u.toUShort()) to CapabilityImpl { args, signals ->
        val urlId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        val id = signals.allocateCell()
        signals.markPending(id)
        requestStore.put(id, HttpRequest(method = "GET", urlId = urlId, parseJson = true))
        id
    }
    // Http.postJson(url, body) (14,3): POST with a JSON body (body id optional).
    entries += CapabilityKey(14u, 3u.toUShort()) to CapabilityImpl { args, signals ->
        val urlId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        val bodyId = (args as? RecordVal)?.fields?.getOrNull(1)?.value?.let { if (it is StrVal) it.id else null }
        val id = signals.allocateCell()
        signals.markPending(id)
        requestStore.put(id, HttpRequest(method = "POST", urlId = urlId, bodyId = bodyId, parseJson = true))
        id
    }

    return entries
}

/** Builds a production `AsyncResolver` that resolves pending `Http` cells. */
public fun makeHttpResolver(
    pending: HttpRequestStore,
    transport: HttpTransport,
    stringResolver: StringResolver,
): AsyncResolver =
    object : AsyncResolver {
        override suspend fun resolve(future: FluxValue): FluxValue {
            val cellId = (future as? FluxValue.IntVal)?.value?.toUInt() ?: 0u
            val request = pending.get(cellId) ?: return NullVal
            pending.remove(cellId)
            val url = stringResolver.resolve(request.urlId)
            val body = request.bodyId?.let { stringResolver.resolve(it) }
            // FLUX-060: broadcast the outbound request to DevTools (guarded by the
            // bridge's DEBUG-gated sink so release builds pay nothing).
            TelemetryBridge.emit(
                TelemetryEvent.NetworkRequest(
                    requestId = cellId,
                    method = request.method,
                    url = url,
                    body = body,
                    capabilityId = CAPABILITY_HTTP,
                ),
            )
            val startedAt = System.currentTimeMillis()
            return try {
                val response = transport.request(request.method, url, body)
                val latencyMs = (System.currentTimeMillis() - startedAt).toUInt().coerceAtMost(UInt.MAX_VALUE)
                TelemetryBridge.emit(
                    TelemetryEvent.NetworkResponse(
                        requestId = cellId,
                        statusCode = 200u,
                        latencyMs = latencyMs,
                        body = response,
                        resultKind = 1u.toUByte(),
                    ),
                )
                if (request.parseJson) {
                    FluxValueJson.parse(response)
                } else {
                    // The response text must reach the VM as a resolvable .str; intern it.
                    StrVal(stringResolver.intern(response))
                }
            } catch (e: Exception) {
                TelemetryBridge.emit(
                    TelemetryEvent.NetworkResponse(
                        requestId = cellId,
                        statusCode = 0u,
                        latencyMs = (System.currentTimeMillis() - startedAt).toUInt().coerceAtMost(UInt.MAX_VALUE),
                        body = e.message,
                        resultKind = 2u.toUByte(),
                    ),
                )
                NullVal
            }
        }
    }
