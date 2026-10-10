package dev.flux.app

import android.content.Context
import android.util.Log
import org.json.JSONObject

/**
 * FLUX-035 (LANE-R, Phase 8) + FLUX-F17 — release crash reporting.
 *
 * A host-native (no webview) reporter that maps a release crash back to the
 * generated component/source, feeding the same [FluxError] shape PRD-K
 * established. RELEASE-ONLY: guarded by `RELEASE` so dev telemetry (ADR-0040)
 * and release crash reporting never mix.
 *
 * ADR-0049 does not apply (new Android-native type).
 *
 * `install()` wires `Thread.setDefaultUncaughtExceptionHandler` so an
 * uncaught exception lands in [lastCrash] with its stack trace and the
 * previous handler still runs. F17 adds resolution of the top frames against
 * an embedded `<stem>.flux.map.json` sidecar (the codegen source map),
 * populating [FluxError.excerpt] with the originating `.flux` file path +
 * 1-based line/col.
 */
public object CrashReporter {
    /** The most recent crash, if any, for display/telemetry. */
    public var lastCrash: FluxError? = null
        private set

    /** A resolved source-map entry (see FLUX-F17). */
    private data class MapEntry(
        val nativeSymbol: String,
        val componentName: String,
        val sourceFile: String,
        val line: UInt,
        val col: UInt,
    )

    /** The embedded source map, or null until [installSourceMap] has run. */
    @Volatile
    private var sourceMap: List<MapEntry>? = null

    /**
     * Loads a codegen source-map sidecar (F17) from the app assets so
     * subsequent crashes resolve their top frames to `.flux` source. The
     * sidecar basename (no extension) matches the generated source stem — e.g.
     * `MainActivity`. Missing or malformed data is silently ignored (a
     * reporter must never crash the app it reports on).
     */
    public fun installSourceMap(context: Context, assetName: String) {
        val path = "$assetName.flux.map.json"
        sourceMap = try {
            context.assets.open(path).bufferedReader().use { it.readText() }.let(::parse)
        } catch (_: Exception) {
            null
        }
    }

    private fun parse(json: String): List<MapEntry> {
        val root = JSONObject(json)
        val arr = root.getJSONArray("component_map")
        val out = ArrayList<MapEntry>(arr.length())
        for (i in 0 until arr.length()) {
            val e = arr.getJSONObject(i)
            out.add(
                MapEntry(
                    nativeSymbol = e.getString("native_symbol"),
                    componentName = e.getString("component_name"),
                    sourceFile = e.getString("source_file"),
                    line = e.getInt("line").toUInt(),
                    col = e.getInt("col").toUInt(),
                ),
            )
        }
        return out
    }

    /** Records a crash as a [FluxError]. Invoked from the installed handler. */
    public fun report(message: String, callSites: List<String> = emptyList()) {
        val excerpt = resolveExcerpt(callSites)
        val err = FluxError(FluxErrorKind.RUNTIME, message, excerpt = excerpt, callSites = callSites)
        lastCrash = err
        Log.e("FluxCrash", err.summary)
    }

    /**
     * Walks the top N frames of the crash and returns the first entry whose
     * `native_symbol` appears in a frame line. Returns null when no map is
     * loaded or no frame matches — the raw [callSites] are always preserved
     * on the error regardless.
     */
    private fun resolveExcerpt(callSites: List<String>): FluxErrorExcerpt? {
        val map = sourceMap ?: return null
        for (frame in callSites.take(8)) {
            for (e in map) {
                if (frame.contains(e.nativeSymbol)) {
                    return FluxErrorExcerpt(
                        path = e.sourceFile,
                        line = e.line,
                        col = e.col,
                        snippet = "",
                    )
                }
            }
        }
        return null
    }

    /** Installs the global uncaught-exception handler. Idempotent. */
    public fun install() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            report(
                message = throwable.message ?: throwable.javaClass.simpleName,
                callSites = throwable.stackTraceToString().lineSequence().toList(),
            )
            previous?.uncaughtException(thread, throwable)
        }
    }
}
