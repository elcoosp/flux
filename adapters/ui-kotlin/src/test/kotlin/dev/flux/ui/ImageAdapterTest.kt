package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ImageAdapterTest {
    @Test
    fun `image adapter writes src and content mode on update`() {
        val adapter = ImageAdapter()
        val view = adapter.create(10u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.IMAGE_SRC to FluxValue.Str("assets/logo.png"),
                PropsIndex.IMAGE_CONTENT_MODE to FluxValue.Str("fit"),
            ),
        )
        assertEquals("assets/logo.png", view.getProperty(ImageAdapter.PROP_SRC))
        assertEquals(true, view.getProperty(ImageAdapter.PROP_HAS_SRC))
        assertEquals("fit", view.getProperty(ImageAdapter.PROP_CONTENT_MODE))
    }

    @Test
    fun `image adapter defaults content mode to fill`() {
        val adapter = ImageAdapter()
        val view = adapter.create(11u)
        adapter.update(view, stringProps(PropsIndex.IMAGE_SRC, "assets/logo.png"))
        assertEquals(
            ImageAdapter.DEFAULT_CONTENT_MODE,
            view.getProperty(ImageAdapter.PROP_CONTENT_MODE),
        )
    }

    @Test
    fun `image adapter forwards width and height when present`() {
        val adapter = ImageAdapter()
        val view = adapter.create(12u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.IMAGE_SRC to FluxValue.Str("assets/logo.png"),
                PropsIndex.IMAGE_WIDTH to FluxValue.Float(120.0),
                PropsIndex.IMAGE_HEIGHT to FluxValue.Float(40.0),
            ),
        )
        assertEquals(120.0, view.getProperty(ImageAdapter.PROP_WIDTH))
        assertEquals(40.0, view.getProperty(ImageAdapter.PROP_HEIGHT))
    }

    @Test
    fun `image adapter degrades safely on missing src`() {
        val adapter = ImageAdapter()
        val view = adapter.create(13u)
        // First set a real source, then simulate the source being removed: the
        // adapter must clear it so the host shows its placeholder (BR-003).
        adapter.update(view, stringProps(PropsIndex.IMAGE_SRC, "assets/logo.png"))
        assertEquals("assets/logo.png", view.getProperty(ImageAdapter.PROP_SRC))

        adapter.update(view, Props.EMPTY)
        assertNull(view.getProperty(ImageAdapter.PROP_SRC))
        assertEquals(false, view.getProperty(ImageAdapter.PROP_HAS_SRC))
    }

    @Test
    fun `destroy clears source to break retain cycle`() {
        val adapter = ImageAdapter()
        val view = adapter.create(14u)
        adapter.update(view, stringProps(PropsIndex.IMAGE_SRC, "assets/logo.png"))
        adapter.destroy(view)
        assertNull(view.getProperty(ImageAdapter.PROP_SRC))
        assertEquals(false, view.getProperty(ImageAdapter.PROP_HAS_SRC))
    }
}
