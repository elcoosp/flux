package dev.flux.app

import android.app.Application
import dev.flux.app.CrashReporter

/** Application singleton for the Flux host (FLUX-035). */
class FluxApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // FLUX-035: install crash reporter; BuildConfig guard removed to avoid AGP 8.7
        // generation issue (BuildConfig not generated when buildFeatures.buildConfig=false).
        // The host's debug vs release behavior is gated elsewhere; always installing is safe
        // for the dev host and restores the build (fixes the revert regression).
        CrashReporter.install()
    }
}
