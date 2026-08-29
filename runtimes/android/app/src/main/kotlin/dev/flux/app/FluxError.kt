package dev.flux.app

/**
 * Android mirror of PRD-K's `FluxError` + `SourceSpan` (crates/flux-types/src/error.rs),
 * consumed by the native error overlay (FLUX-028) and crash reporter (FLUX-035).
 *
 * One error shape across host + DevTools (PRD-O user story 8). The wire carries a
 * span-bearing error field (PRD-K); this is what the host decodes it into.
 * ADR-0049 does not rename these (they are new Android-native types).
 */

/** A source location in a `.flux` file, decoded from a wire `Span` (PRD-K). */
public data class SourceSpan(
    /** The interned source-file id (resolve through the string table). */
    val fileId: UInt,
    /** 1-based line, or 0 when unknown. */
    val line: UInt,
    /** 1-based column, or 0 when unknown. */
    val column: UInt,
)

/** The category of a Flux fault, mirroring `VmErrorKind` + wire/host variants. */
public enum class FluxErrorKind(val raw: String) {
    VM("VmError"),
    WIRE("WireError"),
    RUNTIME("RuntimeError"),
    CAPABILITY("CapabilityError"),
}

/**
 * A Flux fault with a human-readable message, a category, an optional
 * highlighted source span, and a formatted dispatch stack (PRD-K + FLUX-028).
 */
public data class FluxError(
    val message: String,
    val kind: FluxErrorKind,
    val span: SourceSpan? = null,
    val callSites: List<String> = emptyList(),
) {
    /** A one-line summary used by the crash reporter and logs. */
    val summary: String get() = "${kind.raw}: $message"
}
