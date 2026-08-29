package dev.flux.ui

/**
 * Canonical component-local prop indices shared by every adapter in the kit.
 *
 * These MUST match the wire indices the Flux dev server assigns to each
 * component's props (Appendix D / F, and `flux_ir::lower::prop_index_for_name`).
 * The server derives a prop's index from an FNV-1a digest of its *name* (stable
 * across edits, Appendix F §F.0), so a host that read a fixed positional index
 * (e.g. `text = 0`) would never find the value the server placed at the hashed
 * index — which is exactly what produced blank labels. We therefore derive
 * every constant from the same digest, keeping the kit in lockstep with the IR
 * schema and avoiding magic numbers that could drift from the server.
 */
public object PropsIndex {
    /**
     * FNV-1a (32-bit) digest of `name`, masked to `u16` — the exact algorithm
     * `flux_ir::lower::prop_index_for_name` uses to assign a wire prop index.
     * Zero-extends each byte (matching Rust's `u32::from(u8)`) so the host and
     * server compute identical indices for the same name.
     */
    public fun propIndexForName(name: String): UShort {
        var hash: UInt = 0x811c_9dc5u
        for (byte in name.encodeToByteArray()) {
            hash = hash xor byte.toUByte().toUInt()
            hash = hash * 0x0100_0193u
        }
        return (hash and 0xFFFFu).toUShort()
    }

    // Text (F.1)
    public val TEXT_TEXT: UShort = propIndexForName("text")
    public val TEXT_FONT: UShort = propIndexForName("font")
    public val TEXT_SIZE: UShort = propIndexForName("size")
    public val TEXT_COLOR: UShort = propIndexForName("color")
    public val TEXT_ALIGNMENT: UShort = propIndexForName("alignment")
    public val TEXT_MAX_LINES: UShort = propIndexForName("maxLines")
    public val TEXT_OVERFLOW: UShort = propIndexForName("overflow")

    // Button (F.2)
    public val BUTTON_TEXT: UShort = propIndexForName("text")
    public val BUTTON_ON_PRESS: UShort = propIndexForName("onPress")
    public val BUTTON_ENABLED: UShort = propIndexForName("enabled")
    public val BUTTON_COLOR: UShort = propIndexForName("color")

    // Column (F.3) / Row (F.4)
    public val STACK_GAP: UShort = propIndexForName("gap")
    public val STACK_ALIGNMENT: UShort = propIndexForName("alignment")

    // FLUX-037 layout primitives
    public val GRID_COLUMNS: UShort = propIndexForName("columns")
    public val SPACER_FLEX: UShort = propIndexForName("flex")
    public val SAFEAREA_EDGES: UShort = propIndexForName("edges")

    // FLUX-038 overlay containers (`Modal` / `Sheet` / `Dialog`)
    public val OVERLAY_ON_DISMISS: UShort = propIndexForName("onDismiss")

    // FLUX-042 signal-graph animation wrapper (`Animate`)
    public val ANIMATE_SIGNAL: UShort = propIndexForName("signal")
    public val ANIMATE_CURVE: UShort = propIndexForName("curve")
    public val ANIMATE_DURATION: UShort = propIndexForName("duration")

    // TextInput (F.5)
    public val TEXT_INPUT_TEXT: UShort = propIndexForName("text")
    public val TEXT_INPUT_ON_CHANGE_TEXT: UShort = propIndexForName("onChangeText")
    public val TEXT_INPUT_PLACEHOLDER: UShort = propIndexForName("placeholder")
    public val TEXT_INPUT_REF: UShort = propIndexForName("ref")
    public val TEXT_INPUT_ENABLED: UShort = propIndexForName("enabled")
    public val TEXT_INPUT_SECURE_TEXT_ENTRY: UShort = propIndexForName("secureTextEntry")
    public val TEXT_INPUT_KEYBOARD_TYPE: UShort = propIndexForName("keyboardType")

    // Font sub-record
    public val FONT_SIZE: UShort = propIndexForName("size")
    public val FONT_WEIGHT: UShort = propIndexForName("weight")
    public val FONT_FAMILY: UShort = propIndexForName("family")

    // Color sub-record
    public val COLOR_RED: UShort = propIndexForName("red")
    public val COLOR_GREEN: UShort = propIndexForName("green")
    public val COLOR_BLUE: UShort = propIndexForName("blue")
    public val COLOR_ALPHA: UShort = propIndexForName("alpha")

    // Image (F.8)
    public val IMAGE_SOURCE: UShort = propIndexForName("source")
    public val IMAGE_WIDTH: UShort = propIndexForName("width")
    public val IMAGE_HEIGHT: UShort = propIndexForName("height")
    public val IMAGE_RESIZE_MODE: UShort = propIndexForName("resizeMode")

    // Accessibility (FLUX-044) — host-render-only, no wire field. Resolved by
    // name so they stay in lockstep with the dev server's FNV-1a prop indices.
    public val A11Y_LABEL: UShort = propIndexForName("label")
    public val A11Y_ROLE: UShort = propIndexForName("role")
    public val A11Y_FOCUS_ORDER: UShort = propIndexForName("focusOrder")
}
