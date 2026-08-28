package dev.flux.ui

/**
 * An RGBA color with channels in `[0, 1]`, mirroring the `Color` record
 * (`color: Option[Color]`). The Android runtime converts this to
 * an `android.graphics.Color` when binding a `FluxNativeView`.
 */
public data class FluxColor(
    val red: Double,
    val green: Double,
    val blue: Double,
    val alpha: Double,
) {
    /** Encodes this color as a [FluxValue.Record] for the wire/IR. */
    public fun toRecord(): FluxValue.Record =
        FluxValue.Record(
            listOf(
                FluxValue.Field(PropsIndex.COLOR_RED, FluxValue.Float(red)),
                FluxValue.Field(PropsIndex.COLOR_GREEN, FluxValue.Float(green)),
                FluxValue.Field(PropsIndex.COLOR_BLUE, FluxValue.Float(blue)),
                FluxValue.Field(PropsIndex.COLOR_ALPHA, FluxValue.Float(alpha)),
            ),
        )
}

/**
 * A font description, mirroring the `Font` record
 * (`font: Option[Font]`). [weight] and [family] are optional; [size] is the
 * point size in density-independent pixels.
 */
public data class FluxFont(
    val size: Double,
    val weight: String? = null,
    val family: String? = null,
) {
    /** Encodes this font as a [FluxValue.Record] for the wire/IR. */
    public fun toRecord(): FluxValue.Record {
        val entries = mutableListOf<FluxValue.Field>()
        entries.add(FluxValue.Field(PropsIndex.FONT_SIZE, FluxValue.Float(size)))
        if (weight != null) entries.add(FluxValue.Field(PropsIndex.FONT_WEIGHT, FluxValue.Str(weight)))
        if (family != null) entries.add(FluxValue.Field(PropsIndex.FONT_FAMILY, FluxValue.Str(family)))
        return FluxValue.Record(entries)
    }
}
