//  NativeCapabilityHost.swift
//  The injectable seam between the host capability registry and the real device
//  OS (FLUX-045).
//
//  The precompiled host core (`FluxHost`, Foundation-only) must stay free of
//  UIKit/device framework imports so its unit tests run on the simulator without
//  a device — therefore it contains NO real OS calls. The concrete capabilities
//  (Push / Biometric / Background / FileSystem / DeepLink / Sensors, ids 6..=11)
//  forward their `CALL_CAP` through this seam.
//
//  - `DevNativeCapabilityHost` (the default) provides deterministic dev-safe
//    echoes so the dev handshake and the headless round-trip tests stay green
//    with zero OS dependencies.
//  - The app shell (the iOS app target) supplies `IOSNativeCapabilityHost`, which
//    performs the real device work (UNUserNotificationCenter / LAContext /
//    BGTaskScheduler / FileManager / UIApplication / CMMotionManager) behind the
//    same protocol.

import Foundation

/// The contract for a real device-OS capability host. Mirrors `CapabilityImpl`:
/// the implementation receives the `(capId, methodId)`, the call argument, and a
/// mutable view of the live signal store, performs the work (allocating a
/// `Pending` cell and resolving it later for async calls, per ADR-0045), and
/// returns the **result-cell signal id**. A denied grant throws a typed
/// `VmError` (CAPABILITY_DENIED, ADR-0057), never a crash.
public protocol NativeCapabilityHost: Sendable {
    /// True when this host provides a real (non-dev) body for `(capId, methodId)`.
    func handles(_ capId: UInt32, _ methodId: UInt16) -> Bool

    /// Runs the capability body for `(capId, methodId)` and returns the result-cell id.
    /// Only invoked when `handles` is true.
    func call(
        _ capId: UInt32,
        _ methodId: UInt16,
        _ argument: FluxValue,
        _ signals: inout SignalStore
    ) throws -> UInt32
}

/// The default `NativeCapabilityHost`: deterministic dev-safe echoes that need no
/// real OS provider. Mirrors the FLUX-045 wire contract (signal ids 42/43/44 and
/// the 900_000-derived FileSystem id) so the dev handshake and the simulator
/// round-trip tests stay green without a device.
///
/// Real OS behavior lives in `IOSNativeCapabilityHost` (app target).
struct DevNativeCapabilityHost: NativeCapabilityHost {
    func handles(_ capId: UInt32, _ methodId: UInt16) -> Bool {
        (6...11).contains(capId)
    }

    func call(
        _ capId: UInt32,
        _ methodId: UInt16,
        _ argument: FluxValue,
        _ signals: inout SignalStore
    ) throws -> UInt32 {
        switch capId {
        case 6:
            switch methodId {
            case 1:
                // Push.register (6,1) [async]: allocate a Pending cell, resolve inline
                // with a simulated device-token id (signal 42).
                let id = signals.allocateCell()
                signals.markPending(id)
                signals.resolveCell(id, .str(42))
                return id
            case 2:
                // Push.getToken (6,2): surface the last simulated token (signal 42) or null.
                let id = signals.allocateCell()
                signals.write(id, signals.read(42) ?? .null)
                return id
            default:
                throw VmError.typeMismatch(offset: 0)
            }
        case 7:
            switch methodId {
            case 1:
                // Biometric.authenticate (7,1): dev assumes granted; a denied grant MUST
                // yield a typed VmError (CAPABILITY_DENIED), never a crash.
                let id = signals.allocateCell()
                signals.write(id, .bool(true))
                return id
            default:
                throw VmError.typeMismatch(offset: 0)
            }
        case 8:
            switch methodId {
            case 1:
                // Background.schedule (8,1) [async]: allocate a Pending cell, resolve inline
                // with a simulated task id (signal 43).
                let id = signals.allocateCell()
                signals.markPending(id)
                signals.resolveCell(id, .str(43))
                return id
            case 2:
                // Background.cancel (8,2): dev-safe echo.
                let id = signals.allocateCell()
                signals.write(id, .bool(true))
                return id
            default:
                throw VmError.typeMismatch(offset: 0)
            }
        case 9:
            switch methodId {
            case 1:
                // FileSystem.read (9,1): contents persisted under a derived signal id.
                guard case let .record(fields) = argument, !fields.isEmpty else {
                    throw VmError.typeMismatch(offset: 0)
                }
                guard case let .str(pathID) = fields[0].value else {
                    throw VmError.typeMismatch(offset: 0)
                }
                let id = signals.allocateCell()
                signals.write(id, signals.read(Self.fileSignalID(pathID)) ?? .null)
                return id
            case 2:
                // FileSystem.write (9,2): persist into the signal store.
                guard case let .record(fields) = argument, fields.count >= 2 else {
                    throw VmError.typeMismatch(offset: 0)
                }
                guard case let .str(pathID) = fields[0].value else {
                    throw VmError.typeMismatch(offset: 0)
                }
                let data = fields[1].value
                signals.write(Self.fileSignalID(pathID), data)
                let id = signals.allocateCell()
                signals.write(id, data)
                return id
            case 3:
                // FileSystem.delete (9,3): clear the persisted value.
                guard case let .record(fields) = argument, !fields.isEmpty else {
                    throw VmError.typeMismatch(offset: 0)
                }
                guard case let .str(pathID) = fields[0].value else {
                    throw VmError.typeMismatch(offset: 0)
                }
                signals.write(Self.fileSignalID(pathID), .null)
                let id = signals.allocateCell()
                signals.write(id, .null)
                return id
            default:
                throw VmError.typeMismatch(offset: 0)
            }
        case 10:
            switch methodId {
            case 1:
                // DeepLink.openURL (10,1): record the target url (signal 44) for the reconciler.
                signals.write(44, argument)
                return 44
            default:
                throw VmError.typeMismatch(offset: 0)
            }
        case 11:
            switch methodId {
            case 1:
                // Sensors.read (11,1): dev returns an empty record.
                let id = signals.allocateCell()
                signals.write(id, .record([]))
                return id
            default:
                throw VmError.typeMismatch(offset: 0)
            }
        default:
            throw VmError.typeMismatch(offset: 0)
        }
    }

    /// FileSystem contents persisted under a deterministic high signal id.
    private static func fileSignalID(_ pathID: UInt32) -> UInt32 { 900_000 &+ pathID }
}
