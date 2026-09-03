package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test
import java.lang.ref.WeakReference

/**
 * JVM acceptance tests for the FLUX-040 form adapters and the FLUX-041
 * [GestureAdapter]. Each test exercises the adapter against the in-memory
 * [FluxNativeViewImpl] fake so the same code paths that drive real native
 * views are covered on plain JVM (FLUX-009).
 *
 * Conventions mirror [LeafAdapterTest]: the host fires a bound handler by
 * reading the stored `WeakReference<FluxExecutor>` + handler id and dispatching
 * a [HandlerEvent]; the fake records it.
 */
class FormGestureAdapterTest {
    // --- Switch (FLUX-040) ---

    @Test
    fun `switch adapter pushes value and binds onChange`() {
        val adapter = SwitchAdapter.create()
        val view = adapter.create(1u)
        adapter.update(view, propsOf(PropsIndex.SWITCH_VALUE to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(SwitchAdapter.PROP_VALUE))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.SWITCH_ON_CHANGE to FluxValue.HandlerRef(11u)), WeakReference(executor))
        val bound = view.getProperty(SwitchAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(SwitchAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Bool(false)))
        assertEquals(listOf(HandlerEvent(11u, 0u, FluxValue.Bool(false))), executor.events)
    }

    @Test
    fun `switch adapter reflects enabled flag`() {
        val adapter = SwitchAdapter.create()
        val view = adapter.create(2u)
        adapter.update(view, propsOf(PropsIndex.SWITCH_ENABLED to FluxValue.Bool(false)))
        assertEquals(false, view.getProperty(SwitchAdapter.PROP_ENABLED))
        adapter.update(view, propsOf(PropsIndex.SWITCH_ENABLED to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(SwitchAdapter.PROP_ENABLED))
    }

    // --- Toggle (FLUX-077) ---

    @Test
    fun `toggle adapter pushes value and binds onValueChange`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(3u)
        adapter.update(view, propsOf(PropsIndex.TOGGLE_VALUE to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(ToggleAdapter.PROP_VALUE))

        val executor = FluxExecutorFake()
        adapter.bindHandler(
            view,
            propsOf(PropsIndex.TOGGLE_ON_VALUE_CHANGE to FluxValue.HandlerRef(15u)),
            WeakReference(executor),
        )
        val bound = view.getProperty(ToggleAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(ToggleAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Bool(false)))
        assertEquals(listOf(HandlerEvent(15u, 0u, FluxValue.Bool(false))), executor.events)
    }

    @Test
    fun `toggle adapter reflects enabled flag`() {
        val adapter = ToggleAdapter.create()
        val view = adapter.create(2u)
        adapter.update(view, propsOf(PropsIndex.TOGGLE_ENABLED to FluxValue.Bool(false)))
        assertEquals(false, view.getProperty(ToggleAdapter.PROP_ENABLED))
        adapter.update(view, propsOf(PropsIndex.TOGGLE_ENABLED to FluxValue.Bool(true)))
        assertEquals(true, view.getProperty(ToggleAdapter.PROP_ENABLED))
    }

    // --- Checkbox (FLUX-040) ---

    @Test
    fun `checkbox adapter pushes value label and binds onChange`() {
        val adapter = CheckboxAdapter.create()
        val view = adapter.create(3u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.CHECKBOX_VALUE to FluxValue.Bool(true),
                PropsIndex.CHECKBOX_LABEL to FluxValue.Str("Accept"),
            ),
        )
        assertEquals(true, view.getProperty(CheckboxAdapter.PROP_VALUE))
        assertEquals("Accept", view.getProperty(CheckboxAdapter.PROP_LABEL))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.CHECKBOX_ON_CHANGE to FluxValue.HandlerRef(5u)), WeakReference(executor))
        val bound = view.getProperty(CheckboxAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(CheckboxAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Bool(false)))
        assertEquals(listOf(HandlerEvent(5u, 0u, FluxValue.Bool(false))), executor.events)
    }

    // --- Slider (FLUX-040) ---

    @Test
    fun `slider adapter pushes value bounds step and binds onChange`() {
        val adapter = SliderAdapter.create()
        val view = adapter.create(4u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.SLIDER_VALUE to FluxValue.Float(0.5),
                PropsIndex.SLIDER_MIN to FluxValue.Float(0.0),
                PropsIndex.SLIDER_MAX to FluxValue.Float(1.0),
                PropsIndex.SLIDER_STEP to FluxValue.Float(0.1),
            ),
        )
        assertEquals(0.5, view.getProperty(SliderAdapter.PROP_VALUE))
        assertEquals(0.0, view.getProperty(SliderAdapter.PROP_MIN))
        assertEquals(1.0, view.getProperty(SliderAdapter.PROP_MAX))
        assertEquals(0.1, view.getProperty(SliderAdapter.PROP_STEP))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.SLIDER_ON_CHANGE to FluxValue.HandlerRef(8u)), WeakReference(executor))
        val bound = view.getProperty(SliderAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(SliderAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Float(0.8)))
        assertEquals(listOf(HandlerEvent(8u, 0u, FluxValue.Float(0.8))), executor.events)
    }

    // --- Picker (FLUX-040) ---

    @Test
    fun `picker adapter pushes value items and binds onChange`() {
        val adapter = PickerAdapter.create()
        val view = adapter.create(5u)
        val items = FluxValue.List(listOf(FluxValue.Str("a"), FluxValue.Str("b")))
        adapter.update(
            view,
            propsOf(
                PropsIndex.PICKER_VALUE to FluxValue.Int(1L),
                PropsIndex.PICKER_ITEMS to items,
            ),
        )
        assertEquals(1L, view.getProperty(PickerAdapter.PROP_VALUE))
        assertEquals(items, view.getProperty(PickerAdapter.PROP_ITEMS))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.PICKER_ON_CHANGE to FluxValue.HandlerRef(9u)), WeakReference(executor))
        val bound = view.getProperty(PickerAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(PickerAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Int(0L)))
        assertEquals(listOf(HandlerEvent(9u, 0u, FluxValue.Int(0L))), executor.events)
    }

    // --- DatePicker (FLUX-040) ---

    @Test
    fun `date picker adapter pushes value bounds and binds onChange`() {
        val adapter = DatePickerAdapter.create()
        val view = adapter.create(6u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.DATE_PICKER_VALUE to FluxValue.Int(1000L),
                PropsIndex.DATE_PICKER_MIN to FluxValue.Int(0L),
                PropsIndex.DATE_PICKER_MAX to FluxValue.Int(2000L),
            ),
        )
        assertEquals(1000L, view.getProperty(DatePickerAdapter.PROP_VALUE))
        assertEquals(0L, view.getProperty(DatePickerAdapter.PROP_MIN))
        assertEquals(2000L, view.getProperty(DatePickerAdapter.PROP_MAX))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.DATE_PICKER_ON_CHANGE to FluxValue.HandlerRef(12u)), WeakReference(executor))
        val bound = view.getProperty(DatePickerAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(DatePickerAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Int(1500L)))
        assertEquals(listOf(HandlerEvent(12u, 0u, FluxValue.Int(1500L))), executor.events)
    }

    // --- TextArea (FLUX-040) ---

    @Test
    fun `text area adapter pushes value placeholder and binds onChange`() {
        val adapter = TextAreaAdapter.create()
        val view = adapter.create(7u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.TEXT_AREA_VALUE to FluxValue.Str("hello"),
                PropsIndex.TEXT_AREA_PLACEHOLDER to FluxValue.Str("Notes"),
                PropsIndex.TEXT_AREA_MAX_LINES to FluxValue.Int(4L),
            ),
        )
        assertEquals("hello", view.getProperty(TextAreaAdapter.PROP_VALUE))
        assertEquals("Notes", view.getProperty(TextAreaAdapter.PROP_PLACEHOLDER))
        assertEquals(4L, view.getProperty(TextAreaAdapter.PROP_MAX_LINES))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.TEXT_AREA_ON_CHANGE to FluxValue.HandlerRef(6u)), WeakReference(executor))
        val bound = view.getProperty(TextAreaAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(TextAreaAdapter.PROP_HANDLER) as UInt, 0u, FluxValue.Str("updated")))
        assertEquals(listOf(HandlerEvent(6u, 0u, FluxValue.Str("updated"))), executor.events)
    }

    // --- Gesture (FLUX-041) ---

    @Test
    fun `gesture adapter declares kind and binds onGesture`() {
        val adapter = GestureAdapter.create()
        val view = adapter.create(8u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.GESTURE_KIND to FluxValue.Str("longPress"),
                PropsIndex.GESTURE_THRESHOLD to FluxValue.Float(0.5),
            ),
        )
        assertEquals("longPress", view.getProperty(GestureAdapter.PROP_KIND))
        assertEquals(0.5, view.getProperty(GestureAdapter.PROP_THRESHOLD))

        val executor = FluxExecutorFake()
        adapter.bindHandler(view, propsOf(PropsIndex.GESTURE_ON_GESTURE to FluxValue.HandlerRef(21u)), WeakReference(executor))
        val bound = view.getProperty(GestureAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(GestureAdapter.PROP_HANDLER) as UInt, 0u))
        assertEquals(listOf(HandlerEvent(21u, 0u)), executor.events)
    }

    @Test
    fun `gesture adapter reconciles children by stable node id`() {
        val adapter = GestureAdapter.create()
        val view = adapter.create(9u)
        val childA = FluxNativeViewImpl(100u, "text")
        val childB = FluxNativeViewImpl(101u, "text")
        adapter.setChildren(view, listOf(100u, 101u), listOf(childA, childB))
        assertEquals(listOf(100u, 101u), view.children().map { it.nodeId })

        // Reorder: child ids swapped; no view is recreated, only reordered.
        adapter.setChildren(view, listOf(101u, 100u), listOf(childB, childA))
        val reordered = view.children()
        assertEquals(listOf(101u, 100u), reordered.map { it.nodeId })
        assertEquals(childA, reordered[1], "existing view instance must be reused (keyed reconciliation)")
        assertEquals(childB, reordered[0], "existing view instance must be reused (keyed reconciliation)")
    }

    @Test
    fun `gesture adapter stops dispatching after executor disposed`() {
        val adapter = GestureAdapter.create()
        val view = adapter.create(10u)
        val executor = FluxExecutorFake()
        executor.dispose()
        adapter.bindHandler(view, propsOf(PropsIndex.GESTURE_ON_GESTURE to FluxValue.HandlerRef(3u)), WeakReference(executor))
        val bound = view.getProperty(GestureAdapter.PROP_EXECUTOR) as WeakReference<FluxExecutor>
        bound.get()?.dispatch(HandlerEvent(view.getProperty(GestureAdapter.PROP_HANDLER) as UInt, 0u))
        assertEquals(emptyList<HandlerEvent>(), executor.events)
    }

    @Test
    fun `destroy clears bound executor across form and gesture adapters`() {
        for (triple in listOf(
            SwitchAdapter.create() to SwitchAdapter.PROP_EXECUTOR,
            CheckboxAdapter.create() to CheckboxAdapter.PROP_EXECUTOR,
            SliderAdapter.create() to SliderAdapter.PROP_EXECUTOR,
            PickerAdapter.create() to PickerAdapter.PROP_EXECUTOR,
            DatePickerAdapter.create() to DatePickerAdapter.PROP_EXECUTOR,
            TextAreaAdapter.create() to TextAreaAdapter.PROP_EXECUTOR,
            GestureAdapter.create() to GestureAdapter.PROP_EXECUTOR,
        )) {
            val (adapter, executorProp) = triple
            val view = adapter.create(99u)
            val executor = FluxExecutorFake()
            adapter.bindHandler(view, propsOf(), WeakReference(executor))
            adapter.destroy(view)
            assertNull(view.getProperty(executorProp), "destroy must clear the executor ref (FLUX-007)")
        }
    }

    @Test
    fun `kit registers every FLUX-040 and FLUX-041 kind`() {
        for (kind in listOf("switch", "toggle", "checkbox", "slider", "picker", "datepicker", "textarea", "gesture")) {
            val adapter = FluxUiKit.adapterFor(kind)
            assertEquals(true, adapter != null, "kind $kind must resolve to an adapter")
            assertEquals(kind, adapter?.kind)
        }
    }
}
