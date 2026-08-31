package dev.flux.app

import android.app.Application
import dev.flux.app.CrashReporter

/** Application singleton for the Flux host (FLUX-035). */
class FluxApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // FLUX-035: install the release crash reporter so production crashes
        // surface as a FluxError (PRD-R §9) instead of vanishing. Debug builds
        // use ADR-0040 dev telemetry and never mix with the release reporter.
        if (!dev.flux.app.BuildConfig.DEBUG) {
            CrashReporter.install()
        }
    }
}
