package dev.flux.host.wire

import org.junit.jupiter.api.Assertions.assertDoesNotThrow
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

/**
 * FLUX-084: the host wire path must never emit a string id `>=
 * [STRING_ID_CANONICAL_CEILING]`. [assertCanonicalStringId] is the gate applied
 * to every server-assigned `StringInterned` id; a >=ceiling id is a synthetic
 * fallback that must fail loud rather than be placed on the wire (AGENTS.md §3.8).
 */
class StringIdCeilingGuardTest {
    @Test
    fun `canonical id below ceiling passes the guard`() {
        assertDoesNotThrow { assertCanonicalStringId(0x0000_1234u) }
        assertDoesNotThrow { assertCanonicalStringId(STRING_ID_CANONICAL_CEILING - 1u) }
    }

    @Test
    fun `id at ceiling is rejected`() {
        val ex =
            assertThrows(IllegalArgumentException::class.java) {
                assertCanonicalStringId(STRING_ID_CANONICAL_CEILING)
            }
        require(ex.message?.contains("ceiling") == true) { "guard message must name the ceiling: ${ex.message}" }
    }

    @Test
    fun `id above ceiling is rejected`() {
        assertThrows(IllegalArgumentException::class.java) {
            assertCanonicalStringId(0xFFFF_FFFFu)
        }
    }
}
