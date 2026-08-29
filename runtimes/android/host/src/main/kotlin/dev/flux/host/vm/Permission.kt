package dev.flux.host.vm

/**
 * The OS-level permission a capability method requires before `CALL_CAP`
 * resolves on the host (FLUX-049 / LANE-I).
 *
 * A capability method is gated by exactly one [PermissionKind]. The host's
 * permission system (Android `ContextCompat.checkSelfPermission`, iOS
 * `AuthorizationStatus`) answers a single yes/no question per kind, so the gate
 * is intentionally a flat, data-driven table rather than a per-capability
 * closure. Mirrors `flux_types::capabilities::required_permission` 1:1.
 */
public enum class PermissionKind {
    /** Reading the device camera roll / live capture. */
    Camera,

    /** Reading/writing the app's sandboxed file system (NSFileManager / `java.io.File`). */
    FileSystem,

    /** Reading the system pasteboard. */
    Clipboard,

    /** Reading the device's coarse/fine location. */
    Location,

    /** Posting local notifications / registering for push. */
    Notification,

    /** Reading the device biometric enclave (Face ID / fingerprint). */
    Biometric,

    /** Scheduling background work / fetch. */
    Background,

    /** Reading device motion / ambient sensors (iOS `CMMotionManager`; Android `SensorManager`). */
    Sensors,

    /**
     * No OS grant. Used for capabilities whose risk is contained entirely by the
     * host (e.g. in-process state, routed navigation, or a sandboxed WebView) and
     * for escape-valve capabilities that the developer has explicitly opted into
     * at compile time (FLUX-048 / FLUX-046).
     */
    None,

    /** Wrapping an arbitrary native SDK through the `.native` escape hatch
     * (FLUX-046). Gated by an explicit LANE-I allow-list, never an open `CALL_NATIVE`. */
    NativeModule,
}

/**
 * Answers whether a [PermissionKind] has been granted on the host. The production
 * host injects a checker backed by the real OS permission APIs; tests inject a
 * stub. Defaults are provided so the VM compiles without a host context.
 */
public fun interface PermissionChecker {
    public fun isGranted(permission: PermissionKind): Boolean
}

/** Permits every capability. Used by headless VM tests and as the VM default;
 * the production executor overrides it with a real OS-backed checker. */
public val AllowAllPermissionChecker: PermissionChecker = PermissionChecker { _ -> true }

/**
 * The OS permission required by `(capId, methodId)`, or `null` when the
 * capability is unknown to the host (which the `CALL_CAP` gate treats as denied).
 *
 * Must stay byte-for-byte in sync with `flux_types::capabilities::required_permission`
 * and with `stdlib/capabilities.flux` `// requires:` comments. The Rust test
 * `stdlib_requires_match_required_permission` is the single-source-of-truth guard.
 */
public fun requiredPermission(capId: UInt, methodId: UInt): PermissionKind? {
    return when (capId.toUInt()) {
        1u -> PermissionKind.Camera // Camera.takePicture / startPreview / stopPreview
        2u -> PermissionKind.FileSystem // Storage is a sandboxed file write
        3u -> PermissionKind.None // Router.navigate — in-process state swap
        4u -> PermissionKind.Clipboard // Clipboard.setString / getString
        5u -> PermissionKind.Location // Geolocation.getCurrentPosition
        6u -> PermissionKind.Notification // Push.registerForNotifications / scheduleNotification
        7u -> PermissionKind.Biometric // Biometric.authenticate
        8u -> PermissionKind.Background // Background.schedule
        9u -> PermissionKind.FileSystem // FileSystem.readAsString / writeAsString / delete
        10u -> PermissionKind.None // DeepLink.openURL — always permitted
        11u -> PermissionKind.Sensors // Sensors.read
        12u -> PermissionKind.None // WebView.load — sandbox-contained, no OS prompt (FLUX-048)
        13u -> PermissionKind.NativeModule // NativeModule.invoke — explicit .native grant (FLUX-046)
        else -> null
    }.also { _ -> methodId } // methodId is unused today: one permission per capability family
}
