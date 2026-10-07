//  VMDispatchPerfTests.swift
//  Perf #9 — VM dispatch-table option.
//
//  The perf review suggested a LuaJIT-style closure/dispatch table indexed by
//  opcode to replace the `switch` in `FluxBytecodeVM.run`. This test pins
//  `runViaDispatchTable` (the integer-tagged dispatch variant) to be
//  byte-for-byte equivalent to the canonical `run` across a bytecode battery,
//  then micro-benchmarks both. Swift already lowers an `enum` `switch` to a
//  jump table, so the table variant is not expected to be faster; the canonical
//  `run` is retained. The throughput assertion is a regression guard.

import XCTest

@testable import FluxHost

final class VMDispatchPerfTests: XCTestCase {
    /// Every program in the battery must yield an identical `VmOutcome` from
    /// the switch-based `run` and the dispatch-table `runViaDispatchTable`.
    func testDispatchTableMatchesSwitch() async throws {
        let battery: [[UInt8]] = VMDispatchPerfTests.battery()
        for bc in battery {
            var s1: any SignalStore = InMemorySignals()
            var s2: any SignalStore = InMemorySignals()
            // The counterHandler reads signal 1 before writing it back, so seed
            // it to Int(0) in both stores before running — otherwise the very
            // first READ_SIGNAL returns .null and the following ADD_I64 throws
            // a type mismatch. testSwitchThroughput has always done this; the
            // equivalence test was missing the same seed, which made it fail
            // on the first battery program regardless of dispatch path.
            s1.write(1, .int(0))
            s2.write(1, .int(0))
            let a = try FluxBytecodeVM.run(bc, signals: &s1, payload: .null)
            let b = try FluxBytecodeVM.runViaDispatchTable(bc, signals: &s2, payload: .null)
            XCTAssertEqual(a.registers, b.registers, "register mismatch for \(bc)")
            // `signals` is `[(UInt32, FluxValue)]` — an array of tuples, which
            // Swift cannot compare with `==`, so compare element-wise after a
            // stable id sort.
            let sa = a.signals.sorted { $0.0 < $1.0 }
            let sb = b.signals.sorted { $0.0 < $1.0 }
            XCTAssertEqual(sa.count, sb.count, "signal count mismatch for \(bc)")
            for (lhs, rhs) in zip(sa, sb) {
                XCTAssertEqual(lhs.0, rhs.0, "signal id mismatch for \(bc)")
                XCTAssertEqual(lhs.1, rhs.1, "signal value mismatch for \(bc)")
            }
            XCTAssertEqual(a.gasUsed, b.gasUsed, "gas mismatch for \(bc)")
        }
    }

    /// The canonical switch-based evaluator must sustain a throughput budget
    /// (regression guard for the "keep the switch" decision).
    func testSwitchThroughput() async throws {
        let bc = VMDispatchPerfTests.counterHandler()
        let iterations = 20_000
        let start = Date()
        for _ in 0..<iterations {
            var s: any SignalStore = InMemorySignals()
            s.write(1, .int(0))
            _ = try FluxBytecodeVM.run(bc, signals: &s, payload: .null)
        }
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 2.0, "\(iterations) handler evals took \(elapsed)s")
    }

    // MARK: - Bytecode fixtures

    /// A battery exercising arithmetic, control flow, lists, records and caps.
    static func battery() -> [[UInt8]] {
        [
            counterHandler(),
            // LOAD_INT_CONST r0, 7 ; ADD_I64 r0, r0, r0 ; WRITE_SIGNAL 1, r0 ; HALT
            [0xB0, 0x00, 0x07, 0, 0, 0, 0, 0, 0, 0, 0x20, 0x00, 0x00, 0x00, 0x11, 0x01, 0, 0, 0, 0x00],
            // Verified ISA vector (tests/isa-vectors/list_len_basic.json):
            // ALLOC_LIST r0, cap=4 ; LIST_PUSH r0, r1=1 ; LIST_PUSH r0, r2=2 ;
            // LIST_LEN r3, r0. Reuses the exact bytecode the Rust reference VM
            // accepts, so the battery never drifts from the ISA.
            [0x80, 0x00, 0x04, 0x00, 0xb0, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x81, 0x00, 0x01, 0xb0, 0x02, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x81, 0x00, 0x02, 0x83, 0x03, 0x00, 0x00],
        ]
    }

    /// `READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0, r0, r1 ;
    ///  WRITE_SIGNAL 1, r0 ; HALT`.
    static func counterHandler() -> [UInt8] {
        [0x10, 0x00, 0x01, 0, 0, 0,
         0xB0, 0x01, 0x01, 0, 0, 0, 0, 0, 0, 0,
         0x20, 0x00, 0x00, 0x01,
         0x11, 0x01, 0, 0, 0, 0,
         0x00]
    }
    /// The dispatch-table evaluator must be byte-for-byte equivalent to the
    /// switch path across every ISA conformance vector — not just a tiny
    /// battery — so an opcode that exists in only one of the two `switch`
    /// statements is caught immediately (round-25 had 5 such opcodes).
    func testDispatchTableMatchesSwitchOnAllISAVectors() async throws {
        guard let dir = isaVectorsDirectory() else {
            throw XCTSkip("ISA vectors not found; set FLUX_ISA_VECTORS to the directory")
        }
        let candidateUrls = try FileManager.default.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: nil
        ).filter { $0.pathExtension == "json" }.sorted { $0.lastPathComponent < $1.lastPathComponent }

        var tested = 0
        var failures: [String] = []
        for url in candidateUrls {
            guard let data = try? Data(contentsOf: url),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  obj["bytecode_hex"] != nil else { continue }
            let vector = try JSONDecoder().decode(ISAVector.self, from: data)
            let bytecode = hexToBytes(vector.bytecodeHex)

            var s1: any SignalStore = InMemorySignals(store: Dictionary(
                uniqueKeysWithValues: vector.initialSignals.map { ($0.id, toValue($0.value)) }))
            var s2: any SignalStore = InMemorySignals(store: Dictionary(
                uniqueKeysWithValues: vector.initialSignals.map { ($0.id, toValue($0.value)) }))
            let payload = vector.payload.map(toValue) ?? .null

            var strings1 = ConformanceStringTable()
            var strings2 = ConformanceStringTable()
            strings1.seed(vector.strings)
            strings2.seed(vector.strings)

            let ra: Result<VmOutcome, Error> = Result {
                try FluxBytecodeVM.run(bytecode, signals: &s1, payload: payload, stringTable: strings1)
            }
            let rb: Result<VmOutcome, Error> = Result {
                try FluxBytecodeVM.runViaDispatchTable(bytecode, signals: &s2, payload: payload, stringTable: strings2)
            }

            switch (ra, rb) {
            case let (.success(a), .success(b)):
                if !registerArraysMatch(a.registers, b.registers) {
                    failures.append("\(vector.name): registers differ")
                }
                if a.gasUsed != b.gasUsed {
                    failures.append("\(vector.name): gas \(a.gasUsed) vs \(b.gasUsed)")
                }
                let sa = s1.snapshot().sorted { $0.0 < $1.0 }
                let sb = s2.snapshot().sorted { $0.0 < $1.0 }
                if sa.count != sb.count {
                    failures.append("\(vector.name): signal count differ")
                }
                for (l, r) in zip(sa, sb) {
                    if l.0 != r.0 || !fluxValuesEqual(l.1, r.1) {
                        failures.append("\(vector.name): signal \(l.0) differs")
                    }
                }
            case let (.failure(ea), .failure(eb)):
                let a = ea as? VmError
                let b = eb as? VmError
                if a?.kind.name != b?.kind.name {
                    failures.append("\(vector.name): error kind \(a?.kind.name ?? String(describing: ea)) vs \(b?.kind.name ?? String(describing: eb))")
                }
            case let (.success(_), .failure(eb)):
                failures.append("\(vector.name): switch OK but dispatch \(eb)")
            case let (.failure(ea), .success(_)):
                failures.append("\(vector.name): switch \(ea) but dispatch OK")
            }
            tested += 1
        }
        XCTAssertGreaterThan(tested, 0, "no ISA vectors tested")
        XCTAssertTrue(failures.isEmpty,
            "\(failures.count) of \(tested) vectors differ between switch and dispatch table:\n\(failures.joined(separator: "\n"))")
    }
}


// MARK: - Shared vector-loading helpers for the equivalence test

/// Locates the frozen ISA vectors directory (mirrors the ISA conformance
/// test's lookup so both suites load the same corpus).
private func isaVectorsDirectory() -> URL? {
    let env = ProcessInfo.processInfo.environment
    var seeds: [String] = []
    if let override = env["FLUX_ISA_VECTORS"] { seeds.append(override) }
    if let srcroot = env["SRCROOT"] { seeds.append(srcroot) }
    seeds.append(Bundle(for: VMDispatchPerfTests.self).bundleURL.path)
    seeds.append(contentsOf: [
        "../tests/isa-vectors", "tests/isa-vectors",
        "../../tests/isa-vectors", "../../../tests/isa-vectors",
    ])
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

/// Hex string to bytes. Local to this file so it does not collide with the
/// `hexBytes` accessor in ISAConformanceTests.swift.
private func hexToBytes(_ s: String) -> [UInt8] {
    var out: [UInt8] = []
    var iter = s.makeIterator()
    while let hi = iter.next(), let lo = iter.next() {
        guard let h = UInt8(String(hi), radix: 16),
              let l = UInt8(String(lo), radix: 16) else { continue }
        out.append(h << 4 | l)
    }
    return out
}

/// NaN-aware register comparison. `Array<FluxValue> ==` uses `Double.==`,
/// under which two NaNs compare unequal even when their bit patterns match —
/// so the equivalence test must use this helper for vectors that carry NaN
/// (e.g. `eq_f64_nan`, `f64_to_i64_nan`). Non-float values fall back to `==`.
private func fluxValuesEqual(_ a: FluxValue, _ b: FluxValue) -> Bool {
    switch (a, b) {
    case let (.float(x), .float(y)):
        if x.isNaN, y.isNaN { return true }
        return x == y
    case let (.list(xs), .list(ys)):
        return xs.count == ys.count && zip(xs, ys).allSatisfy(fluxValuesEqual)
    case let (.record(xs), .record(ys)):
        return xs.count == ys.count
            && zip(xs, ys).allSatisfy { $0.0 == $1.0 && fluxValuesEqual($0.1, $1.1) }
    default:
        return a == b
    }
}

private func registerArraysMatch(_ a: [FluxValue], _ b: [FluxValue]) -> Bool {
    guard a.count == b.count else { return false }
    return zip(a, b).allSatisfy(fluxValuesEqual)
}
