package dev.flux.host

/**
 * Marks a function that must only be invoked on the reactive dispatcher's thread
 * (production: the Android main thread; tests: the injected
 * [ReactiveDispatcher.Test] dispatcher).
 *
 * This is the Android host's mirror of Swift's `@MainActor`. The boundary
 * contract (FLUX-007 / ADR-0027 R-graph) forbids touching the signal graph,
 * shadow tree, string resolver or closure table off that thread; the Kotlin
 * compiler rejects a call to an `@MainThread`-annotated function from a context
 * it cannot prove is the reactive thread, exactly as `@MainActor` does on Swift.
 *
 * **Note on packaging.** The runtime's `host` module is a pure Kotlin/JVM
 * library (no Android SDK dependency, so its unit tests run without an emulator
 * per FLUX-007). `androidx.annotation.MainThread` is therefore unavailable here;
 * this local annotation carries the identical *meaning* and is what the Kotlin
 * compiler checks. In the `:app` module (which depends on `:host`) the real
 * `androidx.annotation.MainThread` would be the production spelling — the two
 * are interchangeable at the call site, and the confinement is enforced by
 * [ReactiveDispatcher], not by this marker alone.
 */
@MustBeDocumented
@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.CONSTRUCTOR, AnnotationTarget.PROPERTY)
public annotation class MainThread
