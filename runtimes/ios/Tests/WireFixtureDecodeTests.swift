//
//  WireFixtureDecodeTests.swift
//  FluxAppTests — decodes the committed `fixtures/wire/*.bin` binaries through
//  the Swift `FrameDeserializer` (T-505 gate). The FluxHostTests target holds
//  the hand-built-encoder tests; this file covers the *shared fixture* path so
//  the three-decoder gate can run the Swift half via the FluxApp scheme (the
//  FluxHostTests target is not a member of that scheme, and project.yml is
//  frozen per R5).
//

import XCTest

@testable import FluxHost

/// Resolves the `fixtures/wire` directory from the `FLUX_WIRE_FIXTURES`
/// environment variable (set in project.yml for FluxAppTests), then walks up
/// from cwd as a fallback (mirrors ForEachIdDerivationTests' resolution).
private func fixtureDir() -> URL? {
    if let envPath = ProcessInfo.processInfo.environment["FLUX_WIRE_FIXTURES"] {
        let url = URL(fileURLWithPath: envPath)
        if FileManager.default.fileExists(atPath: url.path) { return url }
    }
    var dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath,
        isDirectory: true)
    for _ in 0..<10 {
        let candidate = dir.appendingPathComponent("fixtures/wire")
        if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
        dir = dir.deletingLastPathComponent()
    }
    // Last resort: the test cwd is unknown (iOS sim defaults to /), but the
    // repo root is a sibling of the runtimes/ios project dir. This mirrors the
    // pattern in ForEachIdDerivationTests.
    let repoRoot = URL(fileURLWithPath: "/Users/adm/Documents/Repos/flux")
    let fallback = repoRoot.appendingPathComponent("fixtures/wire")
    if FileManager.default.fileExists(atPath: fallback.path) { return fallback }
    return nil
}

/// Loads a fixture binary by name, returning `nil` (test skip) when the
/// file is absent so the gate degrades cleanly on runners that lack the
/// committed corpus.
private func loadFixture(_ name: String) throws -> [UInt8] {
    let dir = try XCTUnwrap(fixtureDir())
    let url = dir.appendingPathComponent(name)
    guard FileManager.default.fileExists(atPath: url.path) else {
        throw XCTSkip("fixture \(name) not present at \(url.path)")
    }
    let data = try Data(contentsOf: url)
    return Array(data)
}

final class WireFixtureDecodeTests: XCTestCase {
    /// The committed `init_v2.bin` (FLUX-083) must decode successfully and
    /// yield a root node.
    func testInitV2FixtureDecodes() throws {
        let bytes = try loadFixture("init_v2.bin")
        let frame = try FrameDeserializer.decode(bytes)
        XCTAssertNotNil(frame.root, "init_v2 must carry a root node")
    }

    /// The committed `delta_v2.bin` must decode successfully and carry at
    /// least one patch.
    func testDeltaV2FixtureDecodes() throws {
        let bytes = try loadFixture("delta_v2.bin")
        let frame = try FrameDeserializer.decode(bytes)
        XCTAssertFalse(frame.patches.isEmpty, "delta_v2 must carry patches")
    }

    /// The committed `unsupported-version.bin` carries a version byte above
    /// the host's `protocolVersion` (currently `0x04`); every host decoder
    /// must reject it fail-closed as `WireError.unsupportedVersion`.
    func testUnsupportedVersionFixtureRejected() throws {
        let bytes = try loadFixture("unsupported-version.bin")
        XCTAssertEqual(bytes[4], 0x04, "fixture must carry unsupported version 4")
        XCTAssertThrowsError(try FrameDeserializer.decode(bytes)) { error in
            guard let we = error as? WireError,
                  case .unsupportedVersion = we else {
                XCTFail("expected unsupportedVersion, got \(error)"); return
            }
        }
    }

    /// The committed `init_v3.bin` (ADR-0059) must decode through the Swift
    /// decoder's u32 length dispatch and yield a root node.
    func testInitV3FixtureDecodes() throws {
        let bytes = try loadFixture("init_v3.bin")
        XCTAssertEqual(bytes[4], 0x03, "init_v3 must carry protocol version 3")
        let frame = try FrameDeserializer.decode(bytes)
        XCTAssertNotNil(frame.root, "init_v3 must carry a root node")
    }

    /// The committed `delta_v3.bin` must decode and carry at least one patch.
    func testDeltaV3FixtureDecodes() throws {
        let bytes = try loadFixture("delta_v3.bin")
        XCTAssertEqual(bytes[4], 0x03, "delta_v3 must carry protocol version 3")
        let frame = try FrameDeserializer.decode(bytes)
        XCTAssertFalse(frame.patches.isEmpty, "delta_v3 must carry patches")
    }

    /// The v2/v3 pairs share structural content; ADR-0059 widened 16 length
    /// prefixes from u16 to u32, so v3 bytes must be strictly longer. This
    /// pins the widening at the fixture level; a silent reversion to u16
    /// would collapse the two lengths and fail here.
    func testV3FixtureIsWiderThanV2() throws {
        let initV2 = try loadFixture("init_v2.bin")
        let initV3 = try loadFixture("init_v3.bin")
        XCTAssertGreaterThan(initV3.count, initV2.count,
            "v3 Init must be wider than v2: \(initV3.count) vs \(initV2.count)")
        let deltaV2 = try loadFixture("delta_v2.bin")
        let deltaV3 = try loadFixture("delta_v3.bin")
        XCTAssertGreaterThan(deltaV3.count, deltaV2.count,
            "v3 Delta must be wider than v2: \(deltaV3.count) vs \(deltaV2.count)")
    }
}
