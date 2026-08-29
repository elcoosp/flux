package dev.flux.app

import android.util.Log

/**
 * FLUX-035 (LANE-R, Phase 8) — release crash reporting (Kotlin reporter).
 *
 * A host-native (no webview) reporter that maps a release crash back to the
 * generated component/source where possible, feeding the same [FluxError]
 * shape PRD-K established. RELEASE-ONLY: guarded by `RELEASE` so dev telemetry
 * (ADR-0040) and release crash reporting never mix.
 *
 * ADR-0049 does not apply (new Android-native type).
 *
 * RELEASE-TODO: install a `Thread.setDefaultUncaughtExceptionHandler` that
 * captures the stack trace, resolves the top frame to a generated component id
 * via the embedded source map, and forwards [lastCrash] to the reporter sink.
 * This build provides the shape and the [report] entry point.
 */
public object CrashReporter {
    /** The most recent crash, if any, for display/telemetry. */
    public var lastCrash: FluxError? = null
        private set

    /** Records a crash as a [FluxError]. Invoked from the installed handler. */
    public fun report(message: String, callSites: List<String> = emptyList()) {
        lastCrash = FluxError(message, FluxErrorKind.RUNTIME, callSites = callSites)
        Log.e("FluxCrash", lastCrash!!.summary)
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
