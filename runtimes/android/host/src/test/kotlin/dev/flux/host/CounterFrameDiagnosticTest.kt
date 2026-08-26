package dev.flux.host

import dev.flux.host.wire.FrameDeserializer
import org.junit.jupiter.api.Test

/**
 * Diagnostic: deserialize the REAL counter Init frame captured from the dev
 * server and print its structure, so we can verify (without a device) whether
 * the Text node's signal dependencies include the `count` signal. If they do,
 * the dirty-reconcile path will re-materialize the label on a tap; if not, the
 * label freezes at "tapped 0 times".
 */
class CounterFrameDiagnosticTest {
    @Test
    fun `print counter init frame structure`() {
        val bytes =
            CounterFrameDiagnosticTest::class.java
                .classLoader
                .getResourceAsStream("counter_init_frame.bin")!!
                .readBytes()
        val frame = FrameDeserializer.deserialize(bytes)
        println("DIAG fullTree=${frame.fullTree} rootId=${frame.root?.id}")
        println("DIAG handlers=${frame.handlers.size} blobLen=${frame.bytecodeBlob?.len}")
        for (h in frame.handlers) {
            println("DIAG handler id=${h.handlerId} closureOffset=${h.closure.bytecodeOffset} closureLen=${h.closure.bytecodeLen}")
        }
        println("DIAG signalMeta entries=${frame.signalMeta.size}")
        for ((nid, meta) in frame.signalMeta) {
            println("DIAG signalMeta node=$nid deps=${meta.deps} thunkHash=${meta.thunk?.hash?.contentToString()} layout=${meta.layout}")
        }

        // Print each node's component + prop indices so we can see what index
        // `text`, `onClick`, `gap` actually land on.
        fun walk(node: dev.flux.host.wire.WireNode?) {
            if (node == null) return
            println("DIAG node id=${node.id} comp=${node.componentId} props=${node.props.map { (i, v) -> "$i->${v::class.simpleName}" }}")
            for (c in node.children) walk(frame.extraNodes.firstOrNull { it.id == c.nodeId })
        }
        walk(frame.root)
    }
}
