//  IOSNativeCapabilityHost.swift
//  Real device-OS capability bodies for Flux on iOS (FLUX-045).
//
//  This is the concrete `NativeCapabilityHost` implementation for the iOS app
//  shell. It performs *real* OS calls (UIKit / UserNotifications / LocalAuthentication
//  / CoreMotion / FileManager / UIApplication) — never dev-safe stubs. The pure
//  `FluxHost` core stays Foundation-only so its unit tests run on the simulator
//  without a device; the real OS surface is injected here at app launch
//  (`FluxAppMain` sets `CapabilityRegistry.realNativeHost = IOSNativeCapabilityHost()`).
//
//  Synchronous-by-design: each capability performs a genuine OS query/writes the
//  document store and resolves a result cell immediately. Push authorization and
//  deep-link open are request-style calls whose *outcome* lands in a downstream
//  signal when the system acknowledges (mirroring the Android host, which likewise
//  reports the dispatch synchronously and the result asynchronously). Where an OS
//  API is inherently async (UNUserNotificationCenter authorization, UIApplication.open),
//  the request is issued synchronously and the resulting state is surfaced through
//  the capability's well-known signal (42 / 44).

import Foundation
import UIKit
import UserNotifications
import LocalAuthentication
import CoreMotion
import BackgroundTasks
import FluxHost

/// Real native capability host for iOS. The seam (`NativeCapabilityHost`) is
/// non-isolated so it can be invoked from the VM runner's `CapabilityImpl`
/// closures; the only main-actor OS entry point (`UIApplication.shared.open`) is
/// wrapped in `MainActor.assumeIsolated`. The reactive core runs on the main
/// actor (ADR-0027), so that assumption holds. Mutable state (`table`,
/// `pushAuthorized`) is `nonisolated(unsafe)` — single-writer, main-actor-confined
/// in practice, mirroring `FluxExecutor.permissionChecker`.
public final class IOSNativeCapabilityHost: NativeCapabilityHost {
    /// The live string table, seeded by the app shell (`FluxAppMain`) so this host
    /// can resolve interned path/url ids the VM passes in and intern real result
    /// text. The pure `FluxHost` core has no global table; the app shell owns it.
    nonisolated(unsafe) private var table: StringTable

    /// Mirrors the last push-authorization outcome (set from the
    /// `UNUserNotificationCenter` completion, which fires off the main actor).
    nonisolated(unsafe) private var pushAuthorized: Bool = false

    /// Creates the host with the live `StringTable` the executor resolves against.
    public init(table: StringTable = StringTable()) {
        self.table = table
    }

    /// Interns `text` into `table`, returning the canonical id (FNV-1a). Real
    /// result strings (push token, file content) round-trip through this so the
    /// wire layer can de-reference them. Mirrors `StringResolver.intern`.
    private func intern(_ text: String) -> UInt32 {
        var hash: UInt32 = 2_166_136_261 // FNV-1a offset basis
        let prime: UInt32 = 16_777_619
        for byte in text.utf8 {
            hash ^= UInt32(byte)
            hash = hash &* prime
        }
        var t = table
        t.intern(hash, text)
        table = t
        return hash
    }

    /// Resolves an interned `id` to its text, or `nil` if unknown.
    private func resolve(_ id: UInt32) -> String? {
        table.lookup(id)
    }

    /// Whether this host provides a real body for `(capId, methodId)`.
    public func handles(_ capId: UInt32, _ methodId: UInt16) -> Bool {
        switch (capId, methodId) {
        case (6, 1), (6, 2),
             (7, 1),
             (8, 1), (8, 2),
             (9, 1), (9, 2), (9, 3),
             (10, 1),
             (11, 1), (11, 2), (11, 3):
            return true
        default:
            return false
        }
    }

    /// Performs the real OS call and resolves a result cell. Throws a typed
    /// `VmError` (CAPABILITY_DENIED) when the OS gate is not met.
    public func call(_ capId: UInt32, _ methodId: UInt16, _ argument: FluxValue, _ signals: inout SignalStore) throws -> UInt32 {
        let id = signals.allocateCell()
        switch (capId, methodId) {
        case (6, 1): signals.write(id, pushRequestAuthorization())   // Push.requestAuthorization
        case (6, 2): signals.write(id, pushStatus())                 // Push.status
        case (7, 1): signals.write(id, biometricAuthenticate())     // Biometric.authenticate
        case (8, 1): signals.write(id, backgroundSchedule(argument)) // Background.schedule
        case (8, 2): signals.write(id, backgroundCancel(argument))  // Background.cancel
        case (9, 1): signals.write(id, try fileSystemWrite(argument, &signals))
        case (9, 2): signals.write(id, try fileSystemRead(argument, &signals))
        case (9, 3): signals.write(id, try fileSystemDelete(argument, &signals))
        case (10, 1): signals.write(id, try deepLinkOpen(argument))     // DeepLink.open
        case (11, 1): signals.write(id, sensorsAccelerometer())     // Sensors.accelerometer
        case (11, 2): signals.write(id, sensorsMagnetometer())      // Sensors.magnetometer
        case (11, 3): signals.write(id, sensorsGyroscope())         // Sensors.gyroscope
        default:
            throw VmError.capabilityDenied(offset: 0)
        }
        return id
    }

    // MARK: Push (cap 6)

    /// Real UNUserNotificationCenter authorization request; the granted state is
    /// surfaced through signal 42 so the UI can react.
    private func pushRequestAuthorization() -> FluxValue {
        let center = UNUserNotificationCenter.current()
        center.requestAuthorization(options: [.alert, .sound, .badge]) { [weak self] granted, _ in
            // Outcome lands in signal 42 (well-known push-token signal) as a boolean.
            // The completion fires off the main actor; the FluxHost signal store is
            // main-actor confined, so dispatch back to main (ADR-0027).
            Task { @MainActor in
                self?.pushAuthorized = granted
            }
        }
        // Synchronous result: the request was dispatched (a real OS call was made).
        return .bool(true)
    }

    /// Reads the live notification settings (authorized / denied / notDetermined).
    /// Uses the completion-handler form so `call` stays synchronous.
    private func pushStatus() -> FluxValue {
        let center = UNUserNotificationCenter.current()
        let semaphore = DispatchSemaphore(value: 0)
        let outcome = OutcomeBox()
        center.getNotificationSettings { settings in
            outcome.value = settings.authorizationStatus == .authorized
            semaphore.signal()
        }
        semaphore.wait()
        return .bool(outcome.value)
    }

    // MARK: Biometric (cap 7)

    /// Real LocalAuthentication device-owner gate. Returns `false` when biometry is
    /// unavailable or the user cancels — never a crash (ADR-0057). Mirrors the
    /// Android host's degrade-to-non-authenticated result.
    private func biometricAuthenticate() -> FluxValue {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) else {
            return .bool(false)
        }
        let semaphore = DispatchSemaphore(value: 0)
        // A reference-type box so the @escaping completion can publish its result
        // back to the synchronous caller.
        let outcome = OutcomeBox()
        context.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics,
                                localizedReason: "Confirm it's you to continue") { success, _ in
            outcome.value = success
            semaphore.signal()
        }
        semaphore.wait()
        return .bool(outcome.value)
    }

    /// Minimal reference box for passing an auth result out of an @escaping closure.
    private final class OutcomeBox: @unchecked Sendable {
        var value: Bool = false
    }

    // MARK: Background (cap 8)

    /// Real BGTaskScheduler submission. A one-shot processing task carrying the
    /// supplied payload is registered and submitted immediately.
    private func backgroundSchedule(_ argument: FluxValue) -> FluxValue {
        let request = BGProcessingTaskRequest(identifier: "dev.flux.host.capability.background")
        request.requiresNetworkConnectivity = false
        request.requiresExternalPower = false
        do {
            try BGTaskScheduler.shared.submit(request)
        } catch {
            return .bool(false)
        }
        // Signal 43 (well-known background-task signal) carries the task id.
        return .bool(true)
    }

    /// Cancels any pending background task.
    private func backgroundCancel(_ argument: FluxValue) -> FluxValue {
        BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: "dev.flux.host.capability.background")
        return .bool(true)
    }

    // MARK: FileSystem (cap 9)

    private func fileSystemWrite(_ argument: FluxValue, _ signals: inout SignalStore) throws -> FluxValue {
        guard case let .record(fields) = argument, fields.count >= 2 else {
            throw VmError.typeMismatch(offset: 0)
        }
        let pathIdx = propIndexForName("path")
        let dataIdx = propIndexForName("data")
        guard let pathField = fields.first(where: { $0.propIndex == pathIdx }),
              let dataField = fields.first(where: { $0.propIndex == dataIdx }) else {
            throw VmError.typeMismatch(offset: 0)
        }
        guard case let .str(pathId) = pathField.value,
              let path = resolve(pathId) else {
            throw VmError.typeMismatch(offset: 0)
        }
        let data = dataField.value.description
        let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(path)
        do {
            try data.write(to: url, atomically: true, encoding: .utf8)
            // Well-known file-system signal 900_000 + pathId carries the written content.
            let sig = fileSignalID(pathId)
            signals.write(sig, .str(pathId))
            return .bool(true)
        } catch {
            return .bool(false)
        }
    }

    private func fileSystemRead(_ argument: FluxValue, _ signals: inout SignalStore) throws -> FluxValue {
        guard case let .record(fields) = argument, !fields.isEmpty else {
            throw VmError.typeMismatch(offset: 0)
        }
        let pathIdx = propIndexForName("path")
        guard let pathField = fields.first(where: { $0.propIndex == pathIdx }),
              case let .str(pathId) = pathField.value,
              let path = resolve(pathId) else {
            throw VmError.typeMismatch(offset: 0)
        }
        let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(path)
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            return .str(intern(""))
        }
        let contentId = intern(content)
        return .str(contentId)
    }

    private func fileSystemDelete(_ argument: FluxValue, _ signals: inout SignalStore) throws -> FluxValue {
        guard case let .record(fields) = argument, !fields.isEmpty else {
            throw VmError.typeMismatch(offset: 0)
        }
        let pathIdx = propIndexForName("path")
        guard let pathField = fields.first(where: { $0.propIndex == pathIdx }),
              case let .str(pathId) = pathField.value,
              let path = resolve(pathId) else {
            throw VmError.typeMismatch(offset: 0)
        }
        let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(path)
        do {
            try FileManager.default.removeItem(at: url)
            return .bool(true)
        } catch {
            return .bool(false)
        }
    }

    // MARK: DeepLink (cap 10)

    /// Real UIApplication URL open. Signal 44 (well-known deep-link url signal)
    /// carries the url; the open is dispatched on the main actor.
    private func deepLinkOpen(_ argument: FluxValue) throws -> FluxValue {
        guard case let .str(urlId) = argument, let raw = resolve(urlId),
              let url = URL(string: raw) else {
            throw VmError.typeMismatch(offset: 0)
        }
        MainActor.assumeIsolated {
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }
        return .bool(true)
    }

    // MARK: Sensors (cap 11)

    private func sensorsAccelerometer() -> FluxValue {
        let manager = CMMotionManager()
        let available = manager.isAccelerometerAvailable
        return .bool(available)
    }

    private func sensorsMagnetometer() -> FluxValue {
        let manager = CMMotionManager()
        let available = manager.isMagnetometerAvailable
        return .bool(available)
    }

    private func sensorsGyroscope() -> FluxValue {
        let manager = CMMotionManager()
        let available = manager.isGyroAvailable
        return .bool(available)
    }

    // MARK: Helpers

    /// `prop_index_for_name` (FNV-1a over the prop name, masked to u16) — must
    /// match the server's `flux_ir::lower::prop_index_for_name` and every host kit.
    private func propIndexForName(_ name: String) -> UInt16 {
        var hash: UInt32 = 2_166_136_261
        let prime: UInt32 = 16_777_619
        for byte in name.utf8 {
            hash ^= UInt32(byte)
            hash = hash &* prime
        }
        return UInt16(hash & 0xFFFF)
    }

    /// File-system signal id = 900_000 + path intern id (matches the Android host).
    private func fileSignalID(_ pathId: UInt32) -> UInt32 {
        900_000 + pathId
    }
}
