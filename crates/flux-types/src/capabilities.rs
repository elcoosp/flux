//! The single source of truth for the Flux capability surface (spec §24, Appendix E).
//!
//! Every capability's numeric id and method ids are declared exactly once here,
//! and every consumer of capability ids — the IR lower (`CALL_CAP` emission),
//! the dev-server Hello handshake, the native registry codegen, and the
//! capability conformance tests — resolves numeric ids from this table. There is
//! intentionally **no** second, derived id scheme (e.g. hashing names): the
//! compiler and the host runtime must agree on the exact `(cap_id, method_id)`
//! bytes that travel on the wire, and a hash would silently diverge from the
//! small sequential ids the native registries are keyed on.
//!
//! The `stdlib/capabilities.flux` declarations must mirror this table's names;
//! `tests/capability_codegen_parity` fails if a native registry drifts from
//! `CAPABILITY_IDL`.

/// One method on a capability: its wire name and numeric id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodIdl {
    /// The method name as written in `.flux` (e.g. `take`, `navigate`).
    pub name: &'static str,
    /// The numeric method id used by `CALL_CAP`.
    pub id: u16,
}

/// One capability: its wire name, numeric id, and methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityIdl {
    /// The capability name as written in `.flux` and advertised in `Hello`.
    pub name: &'static str,
    /// The numeric capability id used by `CALL_CAP`.
    pub id: u32,
    /// The methods this capability exposes.
    pub methods: &'static [MethodIdl],
}

impl CapabilityIdl {
    /// Resolves a numeric `(cap_id, method_id)` pair to its wire names.
    ///
    /// Returns `None` when the ids are not part of the MLP manifest — a program
    /// that `CALL_CAP`s an unknown id cannot be satisfied by any host.
    #[must_use]
    pub fn names_for(cap_id: u32, method_id: u16) -> Option<(&'static str, &'static str)> {
        let cap = CAPABILITY_IDL.iter().find(|c| c.id == cap_id)?;
        let method = cap.methods.iter().find(|m| m.id == method_id)?;
        Some((cap.name, method.name))
    }

    /// Resolves a capability name to its numeric id, or `None` if the name is
    /// not part of the MLP manifest.
    ///
    /// Used by the IR lower to emit the `CALL_CAP` `cap_id` from the capability
    /// identifier written in `.flux`, so the compiler and runtime stay in lock-
    /// step with [`CAPABILITY_IDL`].
    #[must_use]
    pub fn id_for(name: &str) -> Option<u32> {
        CAPABILITY_IDL.iter().find(|c| c.name == name).map(|c| c.id)
    }

    /// Resolves a `(capability_name, method_name)` pair to its numeric method id,
    /// or `None` if either name is not part of the MLP manifest.
    #[must_use]
    pub fn method_id_for(cap: &str, method: &str) -> Option<u16> {
        let cap = CAPABILITY_IDL.iter().find(|c| c.name == cap)?;
        cap.methods.iter().find(|m| m.name == method).map(|m| m.id)
    }
}

/// The MLP capability set (mirrors `stdlib/capabilities.flux`).
///
/// IDs are stable and match the native `CapabilityRegistry` tables (cap 1 =
/// Camera, cap 2 = Storage, cap 3 = Router, cap 4 = Clipboard, cap 5 =
/// Geolocation). Sync vs async is a binding detail: sync methods return
/// immediately; async methods (most platform calls — camera, permissions,
/// network) resolve through the VM's await machinery (ADR-0044 / ADR-0045) and
/// return a `Result` on failure.
pub const CAPABILITY_IDL: &[CapabilityIdl] = &[
    CapabilityIdl {
        name: "Camera",
        id: 1,
        methods: &[
            MethodIdl {
                name: "takePicture",
                id: 1,
            },
            MethodIdl {
                name: "startPreview",
                id: 2,
            },
            MethodIdl {
                name: "stopPreview",
                id: 3,
            },
        ],
    },
    CapabilityIdl {
        name: "Storage",
        id: 2,
        methods: &[
            MethodIdl {
                name: "setItem",
                id: 1,
            },
            MethodIdl {
                name: "getItem",
                id: 2,
            },
            MethodIdl {
                name: "removeItem",
                id: 3,
            },
        ],
    },
    CapabilityIdl {
        name: "Router",
        id: 3,
        methods: &[MethodIdl {
            name: "navigate",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "Clipboard",
        id: 4,
        methods: &[
            MethodIdl {
                name: "setString",
                id: 1,
            },
            MethodIdl {
                name: "getString",
                id: 2,
            },
        ],
    },
    CapabilityIdl {
        name: "Geolocation",
        id: 5,
        methods: &[MethodIdl {
            name: "getCurrentPosition",
            id: 1,
        }],
    },
    // --- FLUX-045: six concrete native capabilities (PRD-Q deferred set) ---
    // IDs continue the stable sequence; native host bodies are wired in
    // `runtimes/android/host` + `runtimes/ios` (parallel-owned, not here).
    CapabilityIdl {
        name: "Push",
        id: 6,
        methods: &[
            MethodIdl {
                name: "registerForNotifications",
                id: 1,
            },
            MethodIdl {
                name: "scheduleNotification",
                id: 2,
            },
        ],
    },
    CapabilityIdl {
        name: "Biometric",
        id: 7,
        methods: &[MethodIdl {
            name: "authenticate",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "Background",
        id: 8,
        methods: &[MethodIdl {
            name: "schedule",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "FileSystem",
        id: 9,
        methods: &[
            MethodIdl {
                name: "readAsString",
                id: 1,
            },
            MethodIdl {
                name: "writeAsString",
                id: 2,
            },
            MethodIdl {
                name: "delete",
                id: 3,
            },
        ],
    },
    CapabilityIdl {
        name: "DeepLink",
        id: 10,
        methods: &[MethodIdl {
            name: "openURL",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "Sensors",
        id: 11,
        methods: &[MethodIdl {
            name: "read",
            id: 1,
        }],
    },
];

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
    /// Reading/writing the app's sandboxed file system (NSFileManager /
    /// `java.io.File`).
    FileSystem,
    /// Opening external URLs / universal links (always permitted).
    NoneLink,
    /// Reading device motion / ambient sensors (iOS `CMMotionManager`; Android
    /// `SensorManager`).
    Sensors,
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
            Self::NoneLink => ".none",
            Self::Sensors => ".sensors",
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
        _ => None,
    }
}

/// Whether a host's advertised capabilities cover a required
/// `(cap_name, method_name)` pair.
///
/// `advertised` is the `Hello` frame's `capabilities` list
/// `(name, version, features)`. A required method is satisfied when some
/// advertised capability shares its name and lists the method in its features.
#[must_use]
pub fn is_satisfied(
    advertised: &[(String, u32, Vec<String>)],
    cap_name: &str,
    method_name: &str,
) -> bool {
    advertised.iter().any(|(name, _version, features)| {
        name == cap_name && features.iter().any(|f| f == method_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{CapabilityError, FluxError};
    use std::fs;

    /// The stdlib `capabilities.flux` `requires:` annotations must match the
    /// permission the host gates `CALL_CAP` on for each capability id. This is a
    /// single-source-of-truth guard: editing one without the other fails here.
    #[test]
    fn stdlib_requires_match_required_permission() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("stdlib")
            .join("capabilities.flux");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        // Collect `capability <Name>` -> `requires: <token>` pairs from the file.
        let mut current: Option<&str> = None;
        let mut requires: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for line in src.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("capability ") {
                let name = rest.split_whitespace().next().unwrap_or("");
                current = Some(name);
            } else if let Some(rest) = trimmed.strip_prefix("// requires:") {
                if let Some(name) = current {
                    // Only the first whitespace-delimited token is the permission
                    // marker (e.g. `.camera`); the rest is a human description.
                    let token = rest.split_whitespace().next().unwrap_or("").to_owned();
                    requires.insert(name.to_owned(), token);
                }
            }
        }

        for cap in CAPABILITY_IDL {
            let expected = required_permission(cap.id, 0)
                .expect("every manifest capability must declare a permission")
                .token()
                .to_owned();
            let got = requires.get(cap.name).unwrap_or_else(|| {
                panic!(
                    "capabilities.flux missing `// requires:` for `{}`",
                    cap.name
                )
            });
            assert_eq!(
                got, &expected,
                "capabilities.flux `requires` for `{}` ({got}) disagrees with required_permission({}) ({expected})",
                cap.name, cap.id
            );
        }
    }

    #[test]
    fn permission_token_round_trips() {
        assert_eq!(
            PermissionKind::from_token(".camera"),
            Some(PermissionKind::Camera)
        );
        assert_eq!(
            PermissionKind::from_token(".storage"),
            Some(PermissionKind::Storage)
        );
        assert_eq!(
            PermissionKind::from_token(".none"),
            Some(PermissionKind::None)
        );
        assert_eq!(
            PermissionKind::from_token(".clipboard"),
            Some(PermissionKind::Clipboard)
        );
        assert_eq!(
            PermissionKind::from_token(".location"),
            Some(PermissionKind::Location)
        );
        assert_eq!(PermissionKind::Clipboard.token(), ".clipboard");
        assert_eq!(PermissionKind::Location.token(), ".location");
        assert_eq!(PermissionKind::Camera.token(), ".camera");
        assert_eq!(PermissionKind::Storage.token(), ".storage");
        assert_eq!(PermissionKind::None.token(), ".none");
    }

    #[test]
    fn required_permission_matches_manifest() {
        assert_eq!(required_permission(1, 0), Some(PermissionKind::Camera));
        assert_eq!(required_permission(2, 1), Some(PermissionKind::Storage));
        assert_eq!(required_permission(3, 1), Some(PermissionKind::None));
        assert_eq!(required_permission(4, 1), Some(PermissionKind::Clipboard));
        assert_eq!(required_permission(5, 1), Some(PermissionKind::Location));
        assert_eq!(required_permission(99, 99), None);
    }

    /// A test double for [`PermissionChecker`]: returns a fixed grant state.
    struct StubChecker {
        granted: bool,
    }

    impl PermissionChecker for StubChecker {
        fn is_granted(&self, _permission: PermissionKind) -> bool {
            self.granted
        }
    }

    /// The host gate: resolve the required permission for a `CALL_CAP`, then ask
    /// the injected checker. A denied grant must yield a `Capability` error (a
    /// red banner), never a panic or a crash into native code.
    fn gate_call(
        checker: &dyn PermissionChecker,
        cap_id: u32,
        method_id: u16,
    ) -> Result<(), FluxError> {
        let Some(permission) = required_permission(cap_id, method_id) else {
            return Err(crate::error::capability_denied(
                cap_id,
                method_id,
                CapabilityIdl::names_for(cap_id, method_id).map(|(n, _)| n.to_owned()),
                CapabilityIdl::names_for(cap_id, method_id).map(|(_, m)| m.to_owned()),
                "<unknown>".to_owned(),
            ));
        };
        if permission == PermissionKind::None || checker.is_granted(permission) {
            Ok(())
        } else {
            let names = CapabilityIdl::names_for(cap_id, method_id);
            Err(crate::error::capability_denied(
                cap_id,
                method_id,
                names.map(|(n, _)| n.to_owned()),
                names.map(|(_, m)| m.to_owned()),
                permission.token().to_owned(),
            ))
        }
    }

    #[test]
    fn denied_permission_is_capability_error_not_panic() {
        let denied = StubChecker { granted: false };
        let err = gate_call(&denied, 1, 1).expect_err("camera with no grant must be denied");
        match err {
            FluxError::Capability(c) => {
                assert_eq!(c.cap_id, 1);
                assert_eq!(c.method_id, 1);
                assert_eq!(c.required_permission, ".camera");
                assert_eq!(c.cap_name.as_deref(), Some("Camera"));
                assert_eq!(c.method_name.as_deref(), Some("takePicture"));
            }
            other => panic!("expected Capability error, got {other:?}"),
        }
    }

    #[test]
    fn granted_permission_passes_gate() {
        let allowed = StubChecker { granted: true };
        assert!(gate_call(&allowed, 1, 1).is_ok());
    }

    #[test]
    fn router_navigation_never_requires_grant() {
        // `PermissionKind::None` is always reported granted, so navigation passes
        // even with a checker that denies everything else.
        let denied = StubChecker { granted: false };
        assert!(gate_call(&denied, 3, 1).is_ok());
    }

    #[test]
    fn unknown_capability_is_denied() {
        let allowed = StubChecker { granted: true };
        let err = gate_call(&allowed, 99, 1).expect_err("unknown capability must be denied");
        assert!(matches!(err, FluxError::Capability(_)));
    }

    /// FLUX-045: the six concrete native capabilities (Push, Biometric,
    /// Background, FileSystem, DeepLink, Sensors) must be present in the manifest
    /// with stable ids 6..=11 and the permission each resolves through the gate.
    #[test]
    fn flux045_six_concrete_capabilities_wired() {
        let expected = [
            ("Push", 6u32, PermissionKind::Notification),
            ("Biometric", 7, PermissionKind::Biometric),
            ("Background", 8, PermissionKind::Background),
            ("FileSystem", 9, PermissionKind::FileSystem),
            ("DeepLink", 10, PermissionKind::None),
            ("Sensors", 11, PermissionKind::Sensors),
        ];
        for (name, id, perm) in expected {
            let resolved = CapabilityIdl::id_for(name)
                .unwrap_or_else(|| panic!("CAPABILITY_IDL missing {name}"));
            assert_eq!(resolved, id, "stable id for {name}");
            assert_eq!(
                required_permission(id, 1),
                Some(perm),
                "gate permission for {name}"
            );
        }
        // The stdlib declaration must mirror the manifest names (single-source).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("stdlib")
            .join("capabilities.flux");
        let src =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (name, _id, _perm) in expected {
            assert!(
                src.contains(&format!("capability {name} {{")),
                "stdlib/capabilities.flux missing declaration for `{name}`"
            );
        }
    }

    // `CapabilityError` is referenced to keep the import meaningful in this module
    // even if future refactors drop the explicit use above.
    const _: fn() = || {
        let _ = CapabilityError {
            cap_id: 0,
            cap_name: None,
            method_id: 0,
            method_name: None,
            required_permission: String::new(),
            why: String::new(),
        };
    };
}
