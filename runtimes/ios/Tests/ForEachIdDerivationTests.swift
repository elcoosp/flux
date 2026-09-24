//  ForEachIdDerivationTests.swift
//  FA-RENDER Phase B — ForEach row/child id derivation matches the frozen
//  `tests/isa-vectors/foreach_ids.json` vectors (shared with the Kotlin host).
//
//  Also enforces the Phase 3 collision guard: every derived id must carry
//  its marker bits so it never collides with a real server node id.

import Foundation
import XCTest

@testable import FluxHost

/// Cross-platform parity assertion (FLUX-007): the Swift derivation
/// must produce byte-identical ids to the frozen vectors (authored by the
/// reference Rust implementation in `crates/flux-syntax/tests/foreach_ids.rs`).
@MainActor
final class ForEachIdDerivationTests: XCTestCase {

    /// Locates `tests/isa-vectors/` — checks the `FLUX_ISA_VECTORS` env var,
    /// then walks up from cwd, then falls back to the known repo-root path.
    private func vectorDir() -> URL? {
        if let envPath = ProcessInfo.processInfo.environment["FLUX_ISA_VECTORS"] {
            let url = URL(fileURLWithPath: envPath)
            if FileManager.default.fileExists(atPath: url.path) { return url }
        }
        var dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath,
            isDirectory: true)
        for _ in 0..<10 {
            let candidate = dir.appendingPathComponent("tests/isa-vectors")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate
            }
            dir = dir.deletingLastPathComponent()
        }
        // Last resort: the test cwd is unknown, but the repo root is a sibling
        // of the runtimes/ios project dir (xcodebuild cwd defaults to the
        // user's home or the derived data dir).
        let repoRoot = URL(fileURLWithPath: "/Users/adm/Documents/Repos/flux")
        let fallback = repoRoot.appendingPathComponent("tests/isa-vectors")
        if FileManager.default.fileExists(atPath: fallback.path) { return fallback }
        return nil
    }

    /// Loads and parses `foreach_ids.json`, returning the vector array.
    private func loadVectors() -> [[String: Any]] {
        guard let dir = vectorDir() else {
            XCTFail("tests/isa-vectors not found from cwd")
            return []
        }
        let url = dir.appendingPathComponent("foreach_ids.json")
        let data = try! Data(contentsOf: url)
        let json = try! JSONSerialization.jsonObject(with: data) as! [String: Any]
        return (json["vectors"] as? [[String: Any]]) ?? []
    }

    /// Parses a hex string like "0xF7F64E76" into a `UInt32`.
    private func parseHex(_ s: String) -> UInt32 {
        let trimmed = s.hasPrefix("0x") ? String(s.dropFirst(2)) : s
        return UInt32(trimmed, radix: 16)!
    }

    private let reconciler: ShadowTreeReconciler = {
        let table = MaterializationStringTable()
        let registry = AdapterRegistry(table: table)
        return ShadowTreeReconciler(registry: registry, table: table)
    }()

    // MARK: - Vector parity

    func testRowIdMatchesFrozenVectors() {
        for v in loadVectors() {
            let inp = v["input"] as! [String: Any]
            let foreachId = (inp["foreach_id"] as! NSNumber).uint32Value
            // Use splice_key when present, else row_index (T-331).
            let seed: UInt64
            if let key = inp["splice_key"] as? NSNumber {
                seed = key.uint64Value
            } else {
                seed = (inp["row_index"] as! NSNumber).uint64Value
            }
            let expected = parseHex(v["row_id"] as! String)

            let got = reconciler.deriveForEachRowId(foreachId: foreachId, seed: seed)

            XCTAssertEqual(expected, got, "row_id mismatch for input \(inp)")
        }
    }

    func testChildIdMatchesFrozenVectors() {
        for v in loadVectors() {
            let inp = v["input"] as! [String: Any]
            let foreachId = (inp["foreach_id"] as! NSNumber).uint32Value
            let seed: UInt64
            if let key = inp["splice_key"] as? NSNumber {
                seed = key.uint64Value
            } else {
                seed = (inp["row_index"] as! NSNumber).uint64Value
            }
            let origId = (inp["orig_id"] as! NSNumber).uint32Value
            let expected = parseHex(v["child_id"] as! String)

            let rowId = reconciler.deriveForEachRowId(foreachId: foreachId, seed: seed)
            let got = reconciler.deriveForEachChildId(rowId: rowId, origId: origId)

            XCTAssertEqual(expected, got, "child_id mismatch for input \(inp)")
        }
    }

    func testKeyChildIdMatchesFrozenVectors() {
        for v in loadVectors() {
            let inp = v["input"] as! [String: Any]
            guard let keyNum = inp["splice_key"] as? NSNumber,
                  let expectedStr = v["key_child_id"] as? String else { continue }
            let foreachId = (inp["foreach_id"] as! NSNumber).uint32Value
            let key = keyNum.uint64Value
            let expected = parseHex(expectedStr)

            let rowId = reconciler.deriveForEachRowId(foreachId: foreachId, seed: key)
            let got = reconciler.deriveForEachKeyChild(rowId: rowId, key: key)

            XCTAssertEqual(expected, got, "key_child_id mismatch for input \(inp)")
        }
    }

    // MARK: - Collision guard

    func testCollisionGuardAllIdsCarryMarkerBits() {
        let inputs: [(UInt32, UInt64)] = [
            (1, 0), (1, 1), (1, 42), (20, 1), (20, 0),
            (0xFFFFFFFF, 0xFFFFFFFF),
        ]
        for (fid, seed) in inputs {
            let rowId = reconciler.deriveForEachRowId(foreachId: fid, seed: seed)
            XCTAssertNotEqual(0, rowId & 0x8000_0000,
                "row id missing marker bit: 0x\(String(rowId, radix: 16))")

            let childId = reconciler.deriveForEachChildId(rowId: rowId, origId: UInt32(truncating: NSNumber(value: seed)))
            XCTAssertEqual(0xC000_0000, childId & 0xC000_0000,
                "child id missing marker bits: 0x\(String(childId, radix: 16))")

            let keyChildId = reconciler.deriveForEachKeyChild(rowId: rowId, key: seed)
            XCTAssertEqual(0xC000_0000, keyChildId & 0xC000_0000,
                "key-child id missing marker bits: 0x\(String(keyChildId, radix: 16))")
        }
    }

    // MARK: - Divergence

    func testSpliceKeyChangesRowId() {
        let foreachId: UInt32 = 20
        let indexed = reconciler.deriveForEachRowId(foreachId: foreachId, seed: 1)
        let keyed = reconciler.deriveForEachRowId(foreachId: foreachId, seed: 0x0100_0000_0000_0001)
        XCTAssertNotEqual(indexed, keyed, "splice key must change the row id")
    }
}
