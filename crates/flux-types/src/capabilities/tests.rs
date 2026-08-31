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
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    // Collect `capability <Name>` -> `requires: <token>` pairs from the file.
    let mut current: Option<&str> = None;
    let mut requires: std::collections::HashMap<String, String> = std::collections::HashMap::new();
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

// FLUX-046: a user-authored escape-hatch wrapper must derive a stable,
// non-framework-colliding capability id — the server and both hosts compute
// the same bytes for the same module name.
#[test]
fn derive_capability_id_is_deterministic_and_in_user_band() {
    let a = derive_capability_id("StripePayments");
    let b = derive_capability_id("StripePayments");
    assert_eq!(a, b, "must be deterministic for the same name");
    assert!(
        (USER_CAP_BASE..USER_CAP_BASE + 0x1000).contains(&a),
        "must land in the reserved user band, not the framework ids 1..=13"
    );
    let method = derive_method_id("chargeCard");
    assert!(
        (USER_CAP_BASE..USER_CAP_BASE + 0x1000).contains(&(u32::from(method))),
        "derived method id must stay in the user band"
    );
}

#[test]
fn webview_and_native_permission_gate() {
    // WebView is always permitted; NativeModule is gated by the `.native`
    // grant and must be denied without it (the escape hatch is never open).
    let denied = StubChecker { granted: false };
    assert!(
        gate_call(&denied, 12, 1).is_ok(),
        "WebView is always permitted"
    );
    let err = gate_call(&denied, 13, 1).expect_err("NativeModule requires the .native grant");
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
        ("WebView", 12, PermissionKind::None),
        ("NativeModule", 13, PermissionKind::NativeModule),
    ];
    for (name, id, perm) in expected {
        let resolved =
            CapabilityIdl::id_for(name).unwrap_or_else(|| panic!("CAPABILITY_IDL missing {name}"));
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
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
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
