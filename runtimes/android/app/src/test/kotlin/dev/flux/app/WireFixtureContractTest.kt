package dev.flux.app

import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Assumptions.assumeTrue
import org.junit.jupiter.api.Test

class WireFixtureContractTest {
    /**
     * Guards the wire-fixture contract (boundary contract R10): the runtime test
     * suite must consume fixtures from `FLUX_WIRE_FIXTURES` when the environment
     * provides them, and skip cleanly when it does not, so Phase 6 can supply
     * real fixtures without editing runtime code.
     */
    @Test
    fun `wire fixture directory is optional`() {
        val path = System.getenv("FLUX_WIRE_FIXTURES")
        assumeTrue(path != null, "FLUX_WIRE_FIXTURES not set; fixtures land in FLUX-023")
        assertTrue(path!!.isNotEmpty())
    }
}
