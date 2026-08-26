package dev.flux.host

/**
 * Compile-time build flags for the pure-Kotlin `host` runtime.
 *
 * The `host` module is a JVM library (no Android SDK), so the Android
 * `BuildConfig.DEBUG` boolean the spec's trace-gate (brittleness 8d) refers to
 * does not exist here. `BuildFlags.DEBUG` is its exact stand-in: it is a
 * `const val`, so the Kotlin compiler inlines it and **R8 / ProGuard strip the
 * `trace` call sites from a release build** — the same compile-out the spec
 * requires of `BuildConfig.DEBUG` — because the `if (BuildFlags.DEBUG)` branch
 * folds to a constant `false` and the dead `trace?.invoke(...)` is removed.
 *
 * In the `:app` module the production spelling would be `BuildConfig.DEBUG`; the
 * two are behaviourally identical and both feed the same compile-out.
 */
public object BuildFlags {
    /** `true` in debug builds (trace sink live), `false` in release (trace stripped). */
    public const val DEBUG: Boolean = true
}
