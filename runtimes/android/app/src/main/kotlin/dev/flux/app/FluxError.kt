package dev.flux.app

/**
 * Re-exports the unified Flux error shape from [dev.flux.host] so the app module
 * (crash reporter, dev overlay) and any pre-existing call sites keep compiling
 * unchanged.
 *
 * The canonical definitions live in `dev.flux.host.FluxError` (FLUX-075 / ADR-0057):
 * a single `FluxError` model, an eight-value `FluxErrorKind` taxonomy, and the
 * ADR-0057 `excerpt` carrying a server-computed `path:line:col` + snippet so a
 * fault is traceable to `.flux` source on-device.
 */
public typealias FluxError = dev.flux.host.FluxError
public typealias FluxErrorKind = dev.flux.host.FluxErrorKind
public typealias SourceSpan = dev.flux.host.SourceSpan
public typealias FluxErrorExcerpt = dev.flux.host.FluxErrorExcerpt
