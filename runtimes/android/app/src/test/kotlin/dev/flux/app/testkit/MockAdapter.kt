package dev.flux.app.testkit

import dev.flux.ui.FluxAdapter
import dev.flux.ui.FluxExecutor
import dev.flux.ui.FluxNativeView
import dev.flux.ui.HandlerEvent
import dev.flux.ui.Props
import dev.flux.ui.reconcileChildren
import java.lang.ref.WeakReference

/**
 * In-dir mock adapter for the runtime's own integration and E2E tests
 * (FLUX-007 acceptance — runtime tests use in-dir mock adapters rather than the
 * real `adapters/ui-kotlin` dev adapters, which are wired in FLUX-016).
 *
 * It implements the exact [FluxAdapter] contract the production adapters do —
 * [create]/[update]/[setChildren]/[bindHandler]/[destroy] — but records what it
 * was told so tests can assert on the resulting view hierarchy without a real
 * `android.view.View`. This is the same "test double" role `FluxNativeViewImpl`
 * plays in the kit's own tests.
 */
public class MockAdapter(
    override val kind: String,
    private val childCapable: Boolean = true,
) : FluxAdapter<FluxNativeView> {
    /** Records the ordered prop-field indices this adapter has seen via [update]. */
    public val updates: MutableList<Props> = mutableListOf()

    /** Records every [bindHandler] call as a `(nodeId, handlerId)` pair. */
    public val handlerBinds: MutableList<Pair<UInt, UInt>> = mutableListOf()

    /** Records every [create] call's node id. */
    public val created: MutableList<UInt> = mutableListOf()

    /** Records every [destroy] call's node id. */
    public val destroyed: MutableList<UInt> = mutableListOf()

    override fun create(nodeId: UInt): FluxNativeView {
        created.add(nodeId)
        return object : FluxNativeView {
            override val nodeId: UInt = nodeId
            override val kind: String = this@MockAdapter.kind
            private val childViews = mutableListOf<FluxNativeView>()
            private val properties = mutableMapOf<String, Any?>()

            override fun setChildAt(
                index: Int,
                view: FluxNativeView?,
            ) {
                require(index in 0..childViews.size)
                if (view == null) childViews.removeAt(index) else childViews.add(index, view)
            }

            override fun addChild(view: FluxNativeView) {
                childViews.add(view)
            }

            override fun removeChildAt(index: Int): FluxNativeView? = if (index in childViews.indices) childViews.removeAt(index) else null

            override fun children(): List<FluxNativeView> = childViews.toList()

            override fun setProperty(
                property: String,
                value: Any?,
            ): Boolean {
                val previous = properties[property]
                if (previous == value) return false
                properties[property] = value
                return true
            }

            override fun getProperty(property: String): Any? = properties[property]
        }
    }

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        updates.add(props)
        if (kind == "button") {
            props.getHandler(1u) // touch to mirror real adapter reading onClick
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: List<UInt>,
        children: List<FluxNativeView>,
    ) {
        if (!childCapable) return
        reconcileChildren(view, childIds) { id -> children.firstOrNull { it.nodeId == id } }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(1u)
        if (handlerId != 0u) handlerBinds.add(view.nodeId to handlerId)
        // Mirrors the real adapter: a tap routes through the executor. We do not
        // simulate taps here; the binding link is what the E2E test asserts.
        executor.get()?.dispatch(HandlerEvent(handlerId))
    }

    override fun destroy(view: FluxNativeView) {
        destroyed.add(view.nodeId)
        view.clearChildren()
    }
}
