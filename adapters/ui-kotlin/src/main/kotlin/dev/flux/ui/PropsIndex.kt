package dev.flux.ui

/**
 * Canonical component-local prop indices shared by every adapter in the kit.
 *
 * These mirror the field positions the Flux dev server assigns to each
 * component's props (Appendix F). Centralizing them keeps adapters in lockstep
 * with the IR schema and avoids magic numbers at every `props.get(...)`.
 */
public object PropsIndex {
    // Text (F.1)
    public const val TEXT_TEXT: UShort = 0u
    public const val TEXT_FONT: UShort = 1u
    public const val TEXT_SIZE: UShort = 2u
    public const val TEXT_COLOR: UShort = 3u
    public const val TEXT_ALIGNMENT: UShort = 4u
    public const val TEXT_MAX_LINES: UShort = 5u
    public const val TEXT_OVERFLOW: UShort = 6u

    // Button (F.2)
    public const val BUTTON_TEXT: UShort = 0u
    public const val BUTTON_ON_CLICK: UShort = 1u
    public const val BUTTON_ENABLED: UShort = 2u
    public const val BUTTON_COLOR: UShort = 3u

    // Column (F.3) / Row (F.4)
    public const val STACK_GAP: UShort = 0u
    public const val STACK_ALIGNMENT: UShort = 1u

    // TextField (F.5)
    public const val TEXT_FIELD_TEXT: UShort = 0u
    public const val TEXT_FIELD_ON_CHANGE: UShort = 1u
    public const val TEXT_FIELD_PLACEHOLDER: UShort = 2u
    public const val TEXT_FIELD_REF: UShort = 3u
    public const val TEXT_FIELD_ENABLED: UShort = 4u
    public const val TEXT_FIELD_SECURE: UShort = 5u
    public const val TEXT_FIELD_KEYBOARD: UShort = 6u

    // Font sub-record
    public const val FONT_SIZE: UShort = 0u
    public const val FONT_WEIGHT: UShort = 1u
    public const val FONT_FAMILY: UShort = 2u

    // Color sub-record
    public const val COLOR_RED: UShort = 0u
    public const val COLOR_GREEN: UShort = 1u
    public const val COLOR_BLUE: UShort = 2u
    public const val COLOR_ALPHA: UShort = 3u
}
