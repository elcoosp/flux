package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test
import java.lang.ref.WeakReference

class LeafAdapterTest {
    @Test
    fun `text adapter writes text and color on update`() {
        val adapter = TextAdapter.create()
        val view = adapter.create(1u)
        adapter.update(view, stringProps(PropsIndex.TEXT_TEXT, "hello"))
        assertEquals("hello", view.getProperty(TextAdapter.PROP_TEXT))
        assertNull(view.getProperty(TextAdapter.PROP_COLOR))

        adapter.update(view, propsOf(PropsIndex.TEXT_TEXT to FluxValue.Str("bye")))
        assertEquals("bye", view.getProperty(TextAdapter.PROP_TEXT))
    }

    @Test
    fun `button adapter dispatches onClick through weak executor`() {
        val adapter = ButtonAdapter.create()
        val view = adapter.create(2u)
        val executor = FluxExecutorFake()
        adapter.update(view, stringProps(PropsIndex.BUTTON_TEXT, "Tap"))
        adapter.bindHandler(view, propsOf(PropsIndex.BUTTON_ON_CLICK to FluxValue.HandlerRef(7u)), WeakReference(executor))

        // Simulate a tap: the host view fires the bound handler.
        val handlerId = view.getProperty(ButtonAdapter.PROP_HANDLER) as UInt
        val bound = view.getProperty(ButtonAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(handlerId))

        assertEquals(listOf(HandlerEvent(7u)), executor.events)
    }

    @Test
    fun `button adapter stops dispatching after executor disposed`() {
        val adapter = ButtonAdapter.create()
        val view = adapter.create(3u)
        val executor = FluxExecutorFake()
        executor.dispose()
        adapter.bindHandler(view, propsOf(PropsIndex.BUTTON_ON_CLICK to FluxValue.HandlerRef(9u)), WeakReference(executor))
        val handlerId = view.getProperty(ButtonAdapter.PROP_HANDLER) as UInt
        val bound = view.getProperty(ButtonAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(handlerId))
        assertEquals(emptyList<HandlerEvent>(), executor.events)
    }

    @Test
    fun `button adapter reflects enabled flag`() {
        val adapter = ButtonAdapter.create()
        val view = adapter.create(4u)
        adapter.update(view, propsOf(PropsIndex.BUTTON_ENABLED to FluxValue.Bool(false)))
        assertEquals(false, view.getProperty(ButtonAdapter.PROP_ENABLED))
        adapter.update(view, propsOf(PropsIndex.BUTTON_ENABLED to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(ButtonAdapter.PROP_ENABLED))
    }

    @Test
    fun `text field adapter pushes controlled text and binds onChange`() {
        val adapter = TextFieldAdapter.create()
        val view = adapter.create(5u)
        adapter.update(view, stringProps(PropsIndex.TEXT_FIELD_TEXT, "abc"))
        assertEquals("abc", view.getProperty(TextFieldAdapter.PROP_TEXT))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.TEXT_FIELD_ON_CHANGE to FluxValue.HandlerRef(3u)), WeakReference(executor))
        val handlerId = view.getProperty(TextFieldAdapter.PROP_HANDLER) as UInt
        val bound = view.getProperty(TextFieldAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(handlerId, FluxValue.Str("def")))

        assertEquals(listOf(HandlerEvent(3u, FluxValue.Str("def"))), executor.events)
    }

    @Test
    fun `destroy clears bound executor to break retain cycle`() {
        val adapter = ButtonAdapter.create()
        val view = adapter.create(6u)
        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.BUTTON_ON_CLICK to FluxValue.HandlerRef(1u)), WeakReference(executor))
        adapter.destroy(view)
        assertNull(view.getProperty(ButtonAdapter.PROP_EXECUTOR))
        assertEquals(0u, view.getProperty(ButtonAdapter.PROP_HANDLER))
    }
}
