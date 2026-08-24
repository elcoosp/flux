//  FluxValue.swift
//  FluxUIKit — value type mirrored from `flux_syntax::Value` (Appendix C §C.1).

/// A decoded Flux value.
///
/// In the Rust IR `Value::Str` carries a `StringId` (interned). By the time
/// the host's adapter layer receives props, the wire protocol has resolved
/// those IDs to concrete strings, so this kit's `.str` case holds the resolved
/// `String`. Every other case maps 1:1 to the IR `Value` discriminant.
public enum FluxValue: Sendable, Hashable {
    /// Mirrors `Value::Int(i64)`.
    case int(Int64)
    /// Mirrors `Value::Float(f64)`.
    case float(Double)
    /// Mirrors `Value::Bool(bool)`.
    case bool(Bool)
    /// Mirrors `Value::Str(StringId)`, already resolved to the concrete string.
    case str(String)
    /// Mirrors `Value::List(Vec<Value>)`.
    case list([FluxValue])
    /// Mirrors `Value::Record(Vec<(PropIdx, Value)>)`, keyed by prop index.
    case record(Props)
    /// Mirrors `Value::HandlerRef(HandlerId)`.
    case handlerRef(FluxHandlerId)
    /// Mirrors `Value::Null`.
    case null
}
