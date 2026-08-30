package dev.flux.host

/**
 * Android mirror of PRD-K's `FluxError` + `SourceSpan`, the single error shape
 * shared by the on-device overlay (FLUX-028) and the crash reporter (FLUX-035).
 *
 * One error shape across host + DevTools (PRD-O user story 8). The wire carries a
 * span-bearing error field (PRD-K); this is what the host decodes it into.
 *
 * FLUX-075 / ADR-0057 widen this to the eight-value [FluxErrorKind] taxonomy and
 * add [FluxError.excerpt] (a server-computed `path:line:col` + snippet) so a
 * fault is traceable to `.flux` source on-device without a round-trip. The
 * `app` module re-exports these via type aliases (see `dev.flux.app.FluxError`)
 * so the crash reporter and overlay keep compiling unchanged.
 */

/** A source location in a `.flux` file, decoded from a wire span (PRD-K). */
public data class SourceSpan(
    /** The interned source-file id (resolve through the frame's `source_map`). */
    val fileId: UInt,
    /** 1-based line, or 0 when unknown. */
    val line: UInt,
    /** 1-based column, or 0 when unknown. */
    val col: UInt,
)

/**
 * A server-computed source excerpt (ADR-0057) ready for presentation: the
 * resolved file path plus the cited line/column and the offending source line.
 * Built once from the wire [dev.flux.host.wire.FluxErrorExcerpt] by resolving the
 * file id against the frame's `source_map`; never re-derived on the host.
 */
public data class FluxErrorExcerpt(
    /** Resolved source-file path (e.g. `src/Counter.flux`). */
    val path: String,
    /** 1-based line within [path]. */
    val line: UInt,
    /** 1-based column within the cited line. */
    val col: UInt,
    /** The cited source line, trimmed. */
    val snippet: String,
)

/** The category of a Flux fault, mirroring `VmErrorKind` + wire/host variants. */
public enum class FluxErrorKind(val raw: String) {
    PARSE("ParseError"),
    TYPE("TypeError"),
    WIRE("WireError"),
    VM("VmError"),
    RUNTIME("RuntimeError"),
    CAPABILITY("CapabilityError"),
    COMPILE("CompileError"),
    SERVER("ServerError"),
}

/**
 * A Flux fault with a human-readable message, a category, an optional
 * highlighted source span, an optional presentation-ready excerpt, and a
 * formatted dispatch stack (PRD-K + FLUX-028).
 *
 * @property kind the fault category.
 * @property message the what/why/how message (authored server-side, PRD-K §3.11).
 * @property span the source span when known (file id + line + col).
 * @property excerpt the presentation-ready source excerpt when the server
 *   shipped one (ADR-0057); carries the resolved path so the overlay needs no
 *   extra resolution.
 * @property callSites a formatted stack through handler dispatch.
 */
public data class FluxError(
    val kind: FluxErrorKind,
    val message: String,
    val span: SourceSpan? = null,
    val excerpt: FluxErrorExcerpt? = null,
    val callSites: List<String> = emptyList(),
) {
    /** A one-line summary used by the crash reporter and logs. */
    val summary: String get() = "${kind.raw}: $message"
}
