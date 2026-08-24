package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class FluxValuePropsTest {
    @Test
    fun `record field access by index`() {
        val record =
            FluxValue.Record(
                listOf(
                    FluxValue.Field(3u, FluxValue.Str("hi")),
                    FluxValue.Field(7u, FluxValue.Float(1.5)),
                ),
            )
        assertEquals("hi", record.getString(3u))
        assertEquals(1.5, record.getFloat(7u))
        assertNull(record.getString(7u))
    }

    @Test
    fun `props typed accessors decode scalars`() {
        val props =
            propsOf(
                PropsIndex.TEXT_TEXT to FluxValue.Str("hello"),
                PropsIndex.TEXT_COLOR to FluxColor(1.0, 0.0, 0.0, 1.0).toRecord(),
                PropsIndex.BUTTON_ENABLED to FluxValue.Bool(false),
                PropsIndex.BUTTON_ON_CLICK to FluxValue.HandlerRef(42u),
            )
        assertEquals("hello", props.getString(PropsIndex.TEXT_TEXT))
        assertEquals(FluxColor(1.0, 0.0, 0.0, 1.0), props.getColor(PropsIndex.TEXT_COLOR))
        assertEquals(false, props.getBool(PropsIndex.BUTTON_ENABLED, true))
        assertEquals(42u, props.getHandler(PropsIndex.BUTTON_ON_CLICK))
    }

    @Test
    fun `missing handler returns reserved zero`() {
        assertEquals(0u, Props.EMPTY.getHandler(PropsIndex.BUTTON_ON_CLICK))
    }

    @Test
    fun `missing bool falls back to default`() {
        assertTrue(Props.EMPTY.getBool(PropsIndex.BUTTON_ENABLED, true))
        assertEquals(false, Props.EMPTY.getBool(PropsIndex.BUTTON_ENABLED, false))
    }

    @Test
    fun `color decode rejects partial record`() {
        val partial = FluxValue.Record(listOf(FluxValue.Field(PropsIndex.COLOR_RED, FluxValue.Float(1.0))))
        assertNull(Props(listOf(Props.Field(PropsIndex.TEXT_COLOR, partial))).getColor(PropsIndex.TEXT_COLOR))
    }
}
