//  Permission.swift
//  OS-permission gate for CALL_CAP (FLUX-049 / LANE-I).
//
//  Mirrors `flux_types::capabilities::required_permission` and the Android
//  `Permission` module 1:1. A capability method is gated by exactly one
//  `PermissionKind`; the host answers a single yes/no per kind via a
//  `PermissionChecker`. The `CALL_CAP` dispatch sites throw
//  `VmError.capabilityDenied` when the gate fails, which the executor surfaces
//  as a red banner (never a crash).

import Foundation

/// The OS-level permission a capability method requires before `CALL_CAP`
/// resolves on the host.
public enum PermissionKind: Equatable {
    /// Reading the device camera roll / live capture.
    case camera
    /// Reading/writing the app's sandboxed file system.
    case fileSystem
    /// Reading the system pasteboard.
    case clipboard
    /// Reading the device's coarse/fine location.
    case location
    /// Posting local notifications / registering for push.
    case notification
    /// Reading the device biometric enclave (Face ID / fingerprint).
    case biometric
    /// Scheduling background work / fetch.
    case background
    /// Reading device motion / ambient sensors.
    case sensors
    /// No OS grant — in-process state, routed navigation, a sandboxed WebView,
    /// or an explicitly opted-in escape-valve capability.
    case none
    /// Wrapping an arbitrary native SDK through the `.native` escape hatch
    /// (FLUX-046); gated by an explicit LANE-I allow-list.
    case nativeModule
}

/// Answers whether a `PermissionKind` has been granted on the host. The production
/// host injects a checker backed by the real OS permission APIs; tests inject a stub.
public protocol PermissionChecker {
    func isGranted(_ permission: PermissionKind) -> Bool
}

/// Permits every capability. Used by headless VM tests and as the VM default;
/// the production executor overrides it with a real OS-backed checker.
public struct AllowAllPermissionChecker: PermissionChecker {
    public init() {}
    public func isGranted(_ permission: PermissionKind) -> Bool { true }
}

/// Permits no capability. Used by permission-gate tests to prove a denied grant
/// faults rather than panics.
public struct DenyAllPermissionChecker: PermissionChecker {
    public init() {}
    public func isGranted(_ permission: PermissionKind) -> Bool { false }
}

/// The OS permission required by `(capId, methodId)`, or `nil` when the
/// capability is unknown to the host (which the gate treats as denied).
///
/// Must stay byte-for-byte in sync with `flux_types::capabilities::required_permission`
/// and with `stdlib/capabilities.flux` `// requires:` comments.
public func requiredPermission(capID: UInt32, methodID: UInt16) -> PermissionKind? {
    switch capID {
    case 1: .camera // Camera.takePicture / startPreview / stopPreview
    case 2: .fileSystem // Storage reads/writes the sandboxed file system
    case 3: PermissionKind.none // Router.navigate — in-process state swap
    case 4: .clipboard // Clipboard.setString / getString
    case 5: .location // Geolocation.getCurrentPosition
    case 6: .notification // Push.registerForNotifications / scheduleNotification
    case 7: .biometric // Biometric.authenticate
    case 8: .background // Background.schedule
    case 9: .fileSystem // FileSystem.readAsString / writeAsString / delete
    case 10: PermissionKind.none // DeepLink.openURL — always permitted
    case 11: .sensors // Sensors.read
    case 12: PermissionKind.none // WebView.load — sandbox-contained, no OS prompt (FLUX-048)
    case 13: .nativeModule // NativeModule.invoke — explicit .native grant (FLUX-046)
    default: nil
    }
}
