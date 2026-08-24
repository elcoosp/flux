//  Types.swift
//  FluxUIKit — shared type aliases for the Swift adapter kit (FLUX-008).
//
//  These mirror the ID vocabulary in `flux_syntax` (Appendix C §C.1) so the
//  dev runtime (FLUX-006) and this kit agree on the wire shape.

/// Stable node identifier. Mirrors `flux_syntax::NodeId` (`u32`).
public typealias FluxNodeId = UInt32

/// Closure-table index for a bound handler. Mirrors `flux_syntax::HandlerId`.
public typealias FluxHandlerId = UInt32

/// Prop field index within a component's prop map. Mirrors `flux_syntax::PropIdx` (`u16`).
public typealias PropIdx = UInt16
