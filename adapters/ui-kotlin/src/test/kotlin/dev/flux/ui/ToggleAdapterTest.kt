package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import java.lang.ref.WeakReference

/**
 * Mirrors the Swift `ToggleAdapterTests` (FLUX-077): pins the `value` / `enabled`
 * prop mapping, the `onValueChange` handler binding through the weakly-held
 * executor, and dispose-after-release no-op semantics.
 *
 * The native control is driven the same way the Kotlin `SwitchAdapterTest`
 * drives a `UISwitch` — by dispatching a [HandlerEvent] through the bound
 * [FluxExecutor] weak reference.
 */
class ToggleAdapterTest {
    @Test
    fun `toggle pushes value on update`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(1u)
        adapter.update(view, propsOf(PropsIndex.TOGGLE_VALUE to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(ToggleAdapter.PROP_VALUE))

        adapter.update(view, propsOf(PropsIndex.TOGGLE_VALUE to FluxValue.Bool(false)))
        assertEquals(false, view.getProperty(ToggleAdapter.PROP_VALUE))
    }

    @Test
    fun `toggle reflects enabled flag`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(2u)
        adapter.update(view, propsOf(PropsIndex.TOGGLE_ENABLED to FluxValue.Bool(false)))
        assertEquals(false, view.getProperty(ToggleAdapter.PROP_ENABLED))

        adapter.update(view, propsOf(PropsIndex.TOGGLE_ENABLED to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(ToggleAdapter.PROP_ENABLED))
    }

    @Test
    fun `toggle dispatches onValueChange through weak executor`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(3u)
        val executor = FluxExecutorFake()
        adapter.update(view, propsOf(
            PropsIndex.TOGGLE_VALUE to FluxValue.Bool(true),
            PropsIndex.TOGGLE_ENABLED to FluxValue.Bool(true),
        ))
        adapter.bindHandler(
            view,
            propsOf(PropsIndex.TOGGLE_ON_VALUE_CHANGE to FluxValue.HandlerRef(15u)),
            WeakReference(executor),
        )

        // Simulate a user flip: the host view fires the bound handler.
        val handlerId = view.getProperty(ToggleAdapter.PROP_HANDLER) as UInt
        val bound = view.getProperty(ToggleAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(handlerId, 0u, FluxValue.Bool(true)))

        assertEquals(listOf(HandlerEvent(15u, 0u, FluxValue.Bool(true))), executor.events)
    }

    @Test
    fun `toggle stops dispatching after executor disposed`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(4u)
        val executor = FluxExecutorFake()
        executor.dispose()
        adapter.bindHandler(
            view,
            propsOf(PropsIndex.TOGGLE_ON_VALUE_CHANGE to FluxValue.HandlerRef(7u)),
            WeakReference(executor),
        )

        val handlerId = view.getProperty(ToggleAdapter.PROP_HANDLER) as UInt
        val bound = view.getProperty(ToggleAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(handlerId, 0u, FluxValue.Bool(true)))

        assertEquals(emptyList<HandlerEvent>(), executor.events)
    }

    @Test
    fun `toggle destroy clears bound executor`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(5u)
        val executor = FluxExecutorFake()
        adapter.bindHandler(
            view,
            propsOf(PropsIndex.TOGGLE_ON_VALUE_CHANGE to FluxValue.HandlerRef(1u)),
            WeakReference(executor),
        )
        adapter.destroy(view)
        assertNull(view.getProperty(ToggleAdapter.PROP_EXECUTOR))
        assertEquals(0u, view.getProperty(ToggleAdapter.PROP_HANDLER))
    }

    @Test
    fun `toggle resolves from the kit registry`() {
        // The runtime registry (FluxUiKit) wires the `toggle` kind to
        // ToggleAdapter.create(); this pins the factory resolution.
        val adapter = FluxUiKit.adapterFor("toggle")
        assertTrue(adapter != null, "registry must resolve 'toggle'")
        assertTrue(adapter is ToggleAdapter, "resolved adapter must be a ToggleAdapter")
    }
}
