//  ISAConformanceTests.swift
//  ISA conformance target for the Swift VM (FLUX-006).
//
//  Loads every golden vector under `/tests/isa-vectors/` (read-only, shared with
//  `flux-vm-ref`) and asserts the native Swift VM produces the expected signals,
//  registers, errors and gas usage. This is the same suite the Rust reference VM
//  and the Kotlin VM run; divergences fail here.

import XCTest

@testable import FluxApp

final class ISAConformanceTests: XCTestCase {
    /// Locates the frozen ISA vectors directory. Tries, in order: an explicit
    /// `FLUX_ISA_VECTORS` override, then a `SRCROOT`-relative path, then walks
    /// up from the test bundle looking for a `tests/isa-vectors` directory (the
    /// bundle lives under `runtimes/ios/build/...`, so the repo root is a few
    /// levels up). Returns the first candidate whose path actually exists.
    private func vectorsDirectory() -> URL? {
        let env = ProcessInfo.processInfo.environment

        // Seed directories to walk up from: explicit overrides, the SRCROOT
        // project dir, and the test bundle location.
        var seeds: [String] = []
        if let override = env["FLUX_ISA_VECTORS"] { seeds.append(override) }
        if let srcroot = env["SRCROOT"] { seeds.append(srcroot) }
        seeds.append(Bundle(for: ISAConformanceTests.self).bundleURL.path)
        seeds.append(contentsOf: [
            "../tests/isa-vectors", "tests/isa-vectors",
            "../../tests/isa-vectors", "../../../tests/isa-vectors",
        ])

        // From each seed, walk up to six levels looking for a `tests/isa-vectors`
        // directory. Return the first one that really exists.
        for seed in seeds {
            var dir = URL(fileURLWithPath: seed)
            for _ in 0..<7 {
                let candidate = dir.appendingPathComponent("tests/isa-vectors")
                if FileManager.default.fileExists(atPath: candidate.path) {
                    return candidate
                }
                dir = dir.deletingLastPathComponent()
            }
        }
        return nil
    }

    func testAllIsaVectorsPass() throws {
        guard let dir = vectorsDirectory() else {
            throw XCTSkip("ISA vectors not found; set FLUX_ISA_VECTORS to the directory")
        }
        let urls = try FileManager.default.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: nil
        ).filter { $0.pathExtension == "json" }.sorted { $0.lastPathComponent < $1.lastPathComponent }

        XCTAssertFalse(urls.isEmpty, "no vectors loaded from \(dir.path)")

        var passed = 0
        var failures: [String] = []
        for url in urls {
            let data = try Data(contentsOf: url)
            let vector = try JSONDecoder().decode(ISAVector.self, from: data)
            let bytecode = vector.bytecodeHex.hexBytes
            var signals: any SignalStore = InMemorySignals(store: Dictionary(
                uniqueKeysWithValues: vector.initialSignals.map { ($0.id, toValue($0.value)) }
            ))
            let payload = vector.payload.map(toValue) ?? .null

            if let expected = vector.expectedError {
                do {
                    _ = try FluxBytecodeVM.run(bytecode, signals: &signals, payload: payload)
                    failures.append("\(vector.name): expected error \(expected) but succeeded")
                } catch let err as VMError {
                    if err.kind.name != expected.rawValue {
                        failures.append("\(vector.name): expected error \(expected) got \(err.kind.name)")
                    }
                }
            } else {
                let out: VmOutcome
                do {
                    out = try FluxBytecodeVM.run(bytecode, signals: &signals, payload: payload)
                } catch {
                    failures.append("\(vector.name): unexpected error \(error)")
                    continue
                }
                if out.gasUsed != vector.expectedGasUsed {
                    failures.append("\(vector.name): gas \(out.gasUsed) != expected \(vector.expectedGasUsed)")
                }
                for sig in vector.expectedSignals {
                    guard let got = signals.read(sig.id) else {
                        failures.append("\(vector.name): signal \(sig.id) missing")
                        continue
                    }
                    if !valueMatches(got, sig.value) {
                        failures.append("\(vector.name): signal \(sig.id) mismatch: \(got)")
                    }
                }
                for (name, exp) in vector.expectedRegisters {
                    let idx = Int(name.dropFirst())!
                    if !valueMatches(out.registers[idx], exp) {
                        failures.append("\(vector.name): register \(name) mismatch: \(out.registers[idx])")
                    }
                }
            }
            passed += 1
        }

        XCTAssertTrue(
            failures.isEmpty,
            "\(failures.count) of \(urls.count) vectors FAILED:\n\(failures.joined(separator: "\n"))"
        )
        print("conformance: \(passed)/\(urls.count) vectors passed")
    }
}

// MARK: - Vector value conversion

/// Converts a vector value into a `VMValue`. Float strings (`inf`/`-inf`/`nan`)
/// are parsed like the Rust oracle's `parse_float`.
func toValue(_ v: VecValue) -> VMValue {
    switch v.type {
    case "Int": return VMValue.int(v.value?.asInt64 ?? 0)
    case "Float": return VMValue.float(v.value?.asDouble ?? 0.0)
    case "Bool": return VMValue.bool(v.value?.asBool ?? false)
    case "Str": return VMValue.str(UInt32(truncatingIfNeeded: v.value?.asInt64 ?? 0))
    case "Null": return VMValue.null
    case "List":
        let items = v.value?.asArray ?? []
        return .list(items.map { toValue(VecValue(type: $0.asDictionaryType, value: $0)) })
    case "Record":
        let items = v.value?.asArray ?? []
        return .record(items.enumerated().map { (UInt16($0.offset), toValue(VecValue(type: $0.element.asDictionaryType, value: $0.element))) })
    default: return VMValue.null
    }
}

private extension AnyCodable {
    /// The `type` string of a nested value object (`{"type":"Int","value":...}`),
    /// used when flattening List/Record payloads.
    var asDictionaryType: String {
        (value as? [String: AnyCodable])?["type"]?.asString ?? "Null"
    }
}

/// Approximate float comparison matching the Rust oracle's `approx_eq`.
func valueMatches(_ actual: VMValue, _ expected: VecValue) -> Bool {
    let exp = toValue(expected)
    switch (actual, exp) {
    case let (.float(a), .float(b)): return approxEq(a, b)
    case let (.record(a), .record(b)): return a.map(\.1) == b.map(\.1)
    case let (.list(a), .list(b)): return a == b
    default: return actual == exp
    }
}

func approxEq(_ a: Double, _ b: Double) -> Bool {
    if a.isNaN, b.isNaN { return true }
    if a.isInfinite || b.isInfinite { return a == b }
    return (a - b).magnitude < 1e-9
}

private extension StringProtocol {
    /// Hex string -> bytes.
    var hexBytes: [UInt8] {
        var out: [UInt8] = []
        var iter = self.makeIterator()
        while let hi = iter.next(), let lo = iter.next() {
            guard let h = UInt8(String(hi), radix: 16),
                  let l = UInt8(String(lo), radix: 16) else { continue }
            out.append(h << 4 | l)
        }
        return out
    }
}
