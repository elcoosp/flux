package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * Mirrors the Swift `PropsTests.swift` (FLUX-008) for the pure-JVM kit.
 *
 * Pins the typed [Props] accessors: a missing or mistyped field degrades to
 * `null`/default instead of throwing (AGENTS.md §3.5), and the content hash
 * is stable across field reorder (Appendix C §C.1).
 */
class PropsTests {
    @Test
    fun `getString resolves concrete string`() {
        val props = propsOf(PropsIndex.TEXT_TEXT to FluxValue.Str("hello"))
        assertEquals("hello", props.getString(PropsIndex.TEXT_TEXT))
    }

    @Test
    fun `getString returns null for non-string`() {
        val props = propsOf(PropsIndex.TEXT_TEXT to FluxValue.Int(42))
        assertNull(props.getString(PropsIndex.TEXT_TEXT))
    }

    @Test
    fun `getIntFloatBoolHandler round-trip`() {
        val props = propsOf(
            PropsIndex.TEXT_MAX_LINES to FluxValue.Int(-7),
            1u.toUShort() to FluxValue.Float(2.5),
            PropsIndex.BUTTON_ENABLED to FluxValue.Bool(true),
            PropsIndex.BUTTON_ON_PRESS to FluxValue.HandlerRef(9u),
        )
        assertEquals(-7L, props.getInt(PropsIndex.TEXT_MAX_LINES))
        assertEquals(2.5, props.getFloat(1u.toUShort()))
        assertEquals(true, props.getBool(PropsIndex.BUTTON_ENABLED, false))
        assertEquals(9u, props.getHandler(PropsIndex.BUTTON_ON_PRESS))
    }

    @Test
    fun `missing field returns null`() {
        val props = propsOf(0u.toUShort() to FluxValue.Null)
        assertNull(props.getString(99u))
    }

    @Test
    fun `color record decodes positional RGBA`() {
        val color = FluxColor(1.0, 0.0, 0.0, 1.0)
        val props = propsOf(PropsIndex.TEXT_COLOR to color.toRecord())
        val decoded = props.getColor(PropsIndex.TEXT_COLOR)
        assertEquals(1.0, decoded?.red)
        assertEquals(0.0, decoded?.green)
        assertEquals(0.0, decoded?.blue)
        assertEquals(1.0, decoded?.alpha)
    }

    @Test
    fun `font record decodes size weight family`() {
        val font = FluxFont(size = 18.0, weight = "bold", family = "sans-serif")
        val props = propsOf(PropsIndex.TEXT_FONT to font.toRecord())
        val decoded = props.getFont(PropsIndex.TEXT_FONT)
        assertEquals(18.0, decoded?.size)
        assertEquals("bold", decoded?.weight)
        assertEquals("sans-serif", decoded?.family)
    }

    @Test
    fun `prop index is derived from name not position`() {
        // AGENTS.md §3.2: the dev server hashes the prop *name*; the kit must
        // use the same FNV-1a digest or props go silently blank.
        assertEquals(PropsIndex.propIndexForName("text"), PropsIndex.TEXT_TEXT)
        assertEquals(PropsIndex.propIndexForName("color"), PropsIndex.TEXT_COLOR)
        assertEquals(PropsIndex.propIndexForName("onPress"), PropsIndex.BUTTON_ON_PRESS)
        assertEquals(PropsIndex.propIndexForName("value"), PropsIndex.TEXT_INPUT_TEXT)
    }
}
