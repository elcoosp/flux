package dev.flux.host.vm

/**
 * A pending `Http` capability request, stashed by a `CALL_CAP` (cap 14) impl and
 * consumed by the executor's async resolver (ADR-0045).
 *
 * The capability impl runs *without* a string table (it only sees interned ids),
 * so it cannot resolve the URL/body to text itself. Instead it records the
 * request keyed by its result-cell id; the executor's [dev.flux.host.AsyncResolver]
 * — which owns the live string table — later reads the request, resolves the ids
 * to text, performs the real network call, and settles the cell. This is the
 * LANE-C wiring gap (the resolver previously only saw the cell id, never the
 * cap/method/args).
 *
 * @property method the HTTP verb (`GET`/`POST`/`PUT`/`DELETE`).
 * @property urlId the interned id of the request URL.
 * @property bodyId the interned id of the request body, or `null` when absent.
 * @property parseJson when `true` (cap 14, method 2 = `getJson`) the response
 *   body is parsed to a structured [FluxValue]; otherwise it is returned as a
 *   `.str` (the response text interned into the table).
 */
public data class HttpRequest(
    val method: String,
    val urlId: UInt,
    val bodyId: UInt? = null,
    val parseJson: Boolean = false,
)

/**
 * The shared store of in-flight `Http` requests, keyed by the result-cell id the
 * `CALL_CAP` returned. A reference type so the capability impl (which writes)
 * and the executor's async resolver (which reads + performs the call) share one
 * instance — the registry and the resolver must be constructed from the same
 * [HttpRequestStore].
 *
 * Values are safe to share across the single reactive dispatcher the VM runs on
 * (ADR-0027); no internal concurrency is introduced here.
 */
public class HttpRequestStore {
    private val pending = LinkedHashMap<UInt, HttpRequest>()

    /** Records a pending request for [cellId]. */
    public fun put(
        cellId: UInt,
        request: HttpRequest,
    ) {
        pending[cellId] = request
    }

    /** Returns the pending request for [cellId], or `null` if none. */
    public fun get(cellId: UInt): HttpRequest? = pending[cellId]

    /** Removes the pending request for [cellId]. */
    public fun remove(cellId: UInt) {
        pending.remove(cellId)
    }
}

/**
 * The transport seam for `Http` capability resolution (FLUX-047). The executor's
 * async resolver calls [request] with resolved URL/body text and receives the
 * raw response body. The production host supplies [OkHttpTransport]; tests
 * inject a fake to keep the round-trip deterministic without a live server.
 */
public interface HttpTransport {
    /** Performs [method] against [url] with optional [body]; returns the response body. */
    public fun request(
        method: String,
        url: String,
        body: String?,
    ): String
}
