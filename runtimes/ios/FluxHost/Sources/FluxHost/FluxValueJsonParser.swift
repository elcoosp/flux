//  FluxValueJsonParser.swift
//  JSON → `FluxValue` parser (FLUX-047 `Http.getJson` / `postJson` response
//  parsing), built on Foundation's `JSONSerialization` (the same stdlib the
//  host already uses in `VMValueJSON.swift` — no hand-rolled lexer, no new dep).
//
//  Mapping:
//  - JSON object  → `.record` keyed by the field's string index (same shape a
//    struct/record lowers to on the wire), so a `.flux` handler can read fields
//    by `propIndex` after the response is bound.
//  - JSON array   → `.list`.
//  - string       → `.str` carrying the *interned* id. A local FNV-1a interner
//    is used here so the dev/test path produces a stable `UInt32` id; in the
//    running host the executor's resolver interns against the live wire table.
//  - number       → `.int` when integral, otherwise `.float`.
//  - boolean      → `.bool`.
//  - null         → `.null`.
//  A parse failure yields `.null` (a network fault must never crash the host).

import Foundation

enum FluxValueJsonParser {
    /// Parses `text` as JSON, returning a `FluxValue`; `.null` on any error.
    static func parse(_ text: String) -> FluxValue {
        guard let data = text.data(using: .utf8) else { return .null }
        do {
            let obj = try JSONSerialization.jsonObject(with: data, options: [])
            return box(obj)
        } catch {
            return .null
        }
    }

    /// Local FNV-1a-32 interner (dev/test path without a live resolver).
    private static func intern(_ text: String) -> UInt32 {
        var hash: UInt32 = 0x811c_9dc5
        for b in text.utf8 {
            hash ^= UInt32(b)
            hash &*= 0x0100_0193
        }
        return hash & 0x0FFF_FFFF
    }

    private static func box(_ obj: Any) -> FluxValue {
        switch obj {
        case is NSNull:
            return .null
        case let dict as [String: Any]:
            var fields: [(UInt16, FluxValue)] = []
            fields.reserveCapacity(dict.count)
            for (i, (key, value)) in dict.enumerated() {
                fields.append((UInt16(i), .str(intern(key))))
            }
            return .record(fields)
        case let arr as [Any]:
            return .list(arr.map { box($0) })
        case let s as String:
            return .str(intern(s))
        case let n as NSNumber:
            // `NSNumber` collapses bool/int/double; disambiguate via its objC type.
            if CFGetTypeID(n) == CFBooleanGetTypeID() {
                return .bool(n.boolValue)
            }
            let d = n.doubleValue
            if d == Double(n.int64Value) {
                return .int(n.int64Value)
            }
            return .float(d)
        default:
            return .null
        }
    }
}
