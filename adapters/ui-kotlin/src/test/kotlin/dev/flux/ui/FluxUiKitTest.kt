package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class FluxUiKitTest {
    @Test
    fun `adapter contract version matches appendix F`() {
        assertEquals(1, FluxUiKit.ADAPTER_CONTRACT_VERSION)
    }
}
