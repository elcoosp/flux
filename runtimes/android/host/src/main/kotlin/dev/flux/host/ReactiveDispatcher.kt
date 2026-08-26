package dev.flux.host

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers

/**
 * The single dispatcher every thread-confined reactive state mutation runs on.
 *
 * This is the Android host's compile-time enforcement of the R-graph threading
 * contract (ADR-0027 / FLUX-007): the signal graph, shadow tree, string
 * resolver and closure table are **not** thread-safe and must only ever be
 * touched from the dispatcher carried here. Swift mirrors this with
 * `@MainActor`; we mirror it with a [ReactiveDispatcher] value that is injected
 * into [FluxExecutor] and re-binds the plain [CoroutineDispatcher] the coroutine
 * machinery actually needs.
 *
 * The class is `sealed` with exactly two subclasses, [Main] and [Test], each of
 * which exposes its [CoroutineDispatcher] through [dispatcher]. The only way to
 * obtain an instance is the companion factory, so callers cannot construct an
 * arbitrary dispatcher and pretend it is the reactive one — the allowed
 * dispatchers are exactly the production main thread and the test dispatcher.
 *
 * @property dispatcher the underlying [CoroutineDispatcher] stateful work is
 *   confined to. Code that must mutate shared reactive state suspends on this.
 */
public sealed interface ReactiveDispatcher {
    /** The underlying [CoroutineDispatcher] all stateful work is confined to. */
    public val dispatcher: CoroutineDispatcher

    /**
     * The production dispatcher: the Android main thread (UI thread).
     *
     * Mirrors Swift's `@MainActor` — every VM evaluation, signal write and
     * shadow-tree mutation happens here, so native view mutations are always on
     * the thread the platform requires. Constructed only by the companion.
     */
    public class Main private constructor() : ReactiveDispatcher {
        override val dispatcher: CoroutineDispatcher get() = Dispatchers.Main

        internal companion object {
            /** The single production instance. */
            fun create(): Main = Main()
        }
    }

    /**
     * The test dispatcher: a [kotlinx.coroutines.test.StandardTestDispatcher] the
     * unit tests drive deterministically (T12, ADR-0027). It is a real
     * [CoroutineDispatcher] so the executor's `withContext` calls suspend on it,
     * but it is never `Dispatchers.Main` so tests stay on the JVM classpath.
     *
     * Constructed only by the companion (tests call [ReactiveDispatcher.test]).
     *
     * @property dispatcher the injected test dispatcher.
     */
    public class Test private constructor(
        override val dispatcher: CoroutineDispatcher,
    ) : ReactiveDispatcher {
        internal companion object {
            /** Builds a [Test] wrapper around [dispatcher]. */
            fun create(dispatcher: CoroutineDispatcher): Test = Test(dispatcher)
        }
    }

    public companion object {
        /** The production [Main] reactive dispatcher. */
        public fun main(): Main = Main.create()

        /**
         * A [Test] reactive dispatcher wrapping [dispatcher] (the
         * [kotlinx.coroutines.test.StandardTestDispatcher] tests supply).
         */
        public fun test(dispatcher: CoroutineDispatcher): Test = Test.create(dispatcher)
    }
}
