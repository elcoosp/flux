/// The OS-level permission a capability method requires before `CALL_CAP`
/// resolves on the host (LANE-I, FLUX-02X).
///
/// A capability method is gated by exactly one permission. The host checks the
/// OS grant *before* it resolves the call: a denied permission produces a
/// [`FluxError::Capability`](crate::FluxError::Capability) red banner rather
/// than a crash, and the call never reaches the native implementation. The
/// permitted values are a fixed, vetted set so a `.flux` source can only ever
/// request a known grant — there is no escape hatch to an arbitrary OS
/// permission string.
///
/// `stdlib/capabilities.flux` declares each capability's required permission via
/// a `// requires:` annotation, and `tests/capability_permission_parity` fails if
/// the two drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PermissionKind {
    /// Access to the device camera (iOS `NSCameraUsageDescription` /
    /// `AVCaptureDevice`; Android `CAMERA`).
    Camera,
    /// Read/write access to app storage (iOS `PHPhotoLibrary`; Android
    /// `READ_EXTERNAL_STORAGE` / `WRITE_EXTERNAL_STORAGE`).
    Storage,
    /// No OS grant; the router's `navigate` is always permitted.
    None,
    /// Read/write access to the system pasteboard (iOS `UIPasteboard`; Android
    /// `ClipboardManager`).
    Clipboard,
    /// Read access to the device location (iOS `NSLocationWhenInUseUsageDescription`
    /// / `CLLocationManager`; Android `ACCESS_FINE_LOCATION` /
    /// `ACCESS_COARSE_LOCATION`).
    Location,
    /// Posting local/remote notifications (iOS `UNUserNotificationCenter`;
    /// Android `POST_NOTIFICATIONS`).
    Notification,
    /// Local device biometric authentication (iOS `LocalAuthentication`;
    /// Android `BiometricPrompt`).
    Biometric,
    /// Scheduling background work (iOS `BGTaskScheduler`; Android `WorkManager` /
    /// `JobScheduler`).
    Background,
    /// Reading/writing the app's sandboxed file system (NSFileManager /\
    /// `java.io.File`).
    FileSystem,
    /// Reading device motion / ambient sensors (iOS `CMMotionManager`; Android
    /// `SensorManager`).
    Sensors,
    /// Native web content (WKWebView / Android WebView). The escape-hatch
    /// release valve: a `.flux` app may always embed web content, so the host
    /// never prompts — but the threat model (FLUX-049 / ADR-0054) requires the
    /// `src`/`route` to be app-controlled and the webview sandboxed.
    WebView,
    /// Network access for outbound HTTP requests (iOS `NSUrlRequest` /
    /// `URLSession`; Android `INTERNET`). Required by the `Http` capability
    /// (FLUX-047).
    Network,
    /// User-authored native-module escape hatch (FLUX-046): wraps an arbitrary
    /// native SDK as a capability. Always gated by `.native` so a malicious
    /// `.flux` patch cannot invoke an undeclared module — the host must
    /// explicitly allow-list it (LANE-I allow-list, not an open `CALL_NATIVE`).
    NativeModule,
}

impl PermissionKind {
    /// The wire string a `.flux` `requires` annotation uses (the `.camera`
    /// token from the LANE-I spec).
    #[must_use]
    pub fn token(&self) -> &'static str {
        match self {
            Self::Camera => ".camera",
            Self::Storage => ".storage",
            Self::None => ".none",
            Self::Clipboard => ".clipboard",
            Self::Location => ".location",
            Self::Notification => ".notification",
            Self::Biometric => ".biometric",
            Self::Background => ".background",
            Self::FileSystem => ".filesystem",
            Self::Sensors => ".sensors",
            Self::WebView => ".none",
            Self::Network => ".network",
            Self::NativeModule => ".native",
        }
    }

    /// Resolves a `requires` annotation token to a [`PermissionKind`].
    ///
    /// Returns `None` for any token that is not a vetted permission, so the
    /// type checker can reject unknown permissions at compile time rather than
    /// letting them reach the host.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            ".camera" => Some(Self::Camera),
            ".storage" => Some(Self::Storage),
            ".none" => Some(Self::None),
            ".clipboard" => Some(Self::Clipboard),
            ".location" => Some(Self::Location),
            ".notification" => Some(Self::Notification),
            ".biometric" => Some(Self::Biometric),
            ".background" => Some(Self::Background),
            ".filesystem" => Some(Self::FileSystem),
            ".sensors" => Some(Self::Sensors),
            ".network" => Some(Self::Network),
            ".native" => Some(Self::NativeModule),
            _ => None,
        }
    }
}

/// A host-side gate that decides whether an OS permission has been granted for
/// the current process (LANE-I, FLUX-02X).
///
/// The production hosts inject a real checker (iOS
/// `AVCaptureDevice.authorizationStatus` / `PHPhotoLibrary`; Android
/// `ContextCompat.checkSelfPermission`); tests inject a stub. The registry
/// closure calls [`PermissionChecker::is_granted`] *before* resolving a
/// `CALL_CAP`, so a denied permission surfaces as a `Capability` error (variant
/// of [`crate::error::FluxError`]) and never reaches native code.
///
/// The checker is per-`(cap_id, method_id)` because the grant required depends
/// on which capability method is being invoked.
pub trait PermissionChecker: Send + Sync {
    /// Returns `true` when the OS has granted the permission required by
    /// `permission`.
    ///
    /// May be async on the host (a system prompt), but the checker itself is a
    /// synchronous probe of *current* status — the async prompt-and-await lives
    /// in the capability's resolution path, not here.
    fn is_granted(&self, permission: PermissionKind) -> bool;
}

/// The permission required to invoke a capability method, resolved from the
/// authoritative [`CAPABILITY_IDL`] table (LANE-I, FLUX-02X).
///
/// This is the single source of truth the host consults before resolving a
/// `CALL_CAP`: pass the `(cap_id, method_id)` from the wire, and gate the call
/// on [`PermissionChecker::is_granted`] of the returned [`PermissionKind`].
/// Unknown `(cap_id, method_id)` pairs return `None`, which the host treats as a
/// denial (the capability is not part of the MLP manifest).
#[must_use]
pub fn required_permission(cap_id: u32, method_id: u16) -> Option<PermissionKind> {
    match (cap_id, method_id) {
        // Camera: every method needs the camera grant.
        (1, _) => Some(PermissionKind::Camera),
        // Storage: every method needs the storage grant.
        (2, _) => Some(PermissionKind::Storage),
        // Router: navigation is always permitted.
        (3, _) => Some(PermissionKind::None),
        // Clipboard: every method needs the pasteboard grant.
        (4, _) => Some(PermissionKind::Clipboard),
        // Geolocation: every method needs the location grant.
        (5, _) => Some(PermissionKind::Location),
        // Push: posting notifications (async, ADR-0045 AsyncResolver).
        (6, _) => Some(PermissionKind::Notification),
        // Biometric: local device authentication.
        (7, _) => Some(PermissionKind::Biometric),
        // Background: scheduling background work.
        (8, _) => Some(PermissionKind::Background),
        // FileSystem: sandboxed file read/write/delete.
        (9, _) => Some(PermissionKind::FileSystem),
        // DeepLink: opening URLs is always permitted.
        (10, _) => Some(PermissionKind::None),
        // Sensors: device motion / ambient sensors.
        (11, _) => Some(PermissionKind::Sensors),
        // WebView: native web content is always permitted (escape valve) — no OS
        // grant prompt; the risk is contained by sandboxing (FLUX-049 threat model).
        (12, _) => Some(PermissionKind::None),
        // NativeModule: user escape-hatch wrapper — always gated by `.native`
        // so only host-allow-listed modules resolve (no open CALL_NATIVE).
        (13, _) => Some(PermissionKind::NativeModule),
        // Http: outbound network requests need the network grant (FLUX-047).
        (14, _) => Some(PermissionKind::Network),
        // Persist: structured local persistence reuses the storage grant
        // (FLUX-047) — sandboxed app data, same OS gate as key-value Storage.
        (15, _) => Some(PermissionKind::Storage),
        _ => None,
    }
}
