package dev.flux.app

import dev.flux.app.testkit.IsaConformance
import org.junit.jupiter.api.DynamicTest
import org.junit.jupiter.api.TestFactory

/**
 * ISA conformance test target for the Kotlin `FluxBytecodeVM` (FLUX-007).
 *
 * Loads every golden vector from `/tests/isa-vectors` (read-only JSON) and
 * asserts the Kotlin VM agrees with the Rust `flux-vm-ref` oracle on
 * signals/registers/errors/gas. This is the FLUX-007 acceptance gate: all 71
 * vectors must pass on the Kotlin VM.
 */
class IsaConformanceTest {
    @TestFactory
    fun allIsaVectorsPass(): List<DynamicTest> = IsaConformance.dynamicTests()
}
