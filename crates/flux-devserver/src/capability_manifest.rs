//! The canonical capability manifest (spec §24, ADR-0045).
//!
//! Capabilities are the only sanctioned way `.flux` reaches native APIs. The
//! wire `Hello` handshake advertises them by *name* (`("Camera", 1,
//! ["take", ...])`), but the lowered bytecode refers to them by *numeric id*
//! (`CALL_CAP cap_id, method_id`). This manifest is the single source of truth
//! mapping the two, so the dev server can check a host's advertised set against
//! what the compiled tree actually `CALL_CAP`s.
//!
//! The ids/names here are stable and must stay in lock-step with
//! `stdlib/capabilities.flux` and the native `CapabilityRegistry` tables
//! (iOS `Registry.swift`, Android `CapabilityRegistry.kt`). When the capability
//! IDL lands (ADR-0045), this table becomes generated rather than hand-written.

/// A single method on a capability: its wire name and numeric id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ManifestMethod {
    /// The method name as advertised in `Hello` and written in `.flux`.
    pub(crate) name: &'static str,
    /// The numeric method id used by `CALL_CAP`.
    pub(crate) id: u16,
}

/// A capability: its wire name, numeric id, and methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ManifestCapability {
    /// The capability name as advertised in `Hello` and written in `.flux`.
    pub(crate) name: &'static str,
    /// The numeric capability id used by `CALL_CAP`.
    pub(crate) id: u32,
    /// The methods this capability exposes.
    pub(crate) methods: &'static [ManifestMethod],
}

/// The MLP capability set (mirrors `stdlib/capabilities.flux`).
pub(crate) const CAPABILITIES: &[ManifestCapability] = &[
    ManifestCapability {
        name: "Camera",
        id: 1,
        methods: &[
            ManifestMethod {
                name: "take",
                id: 1,
            },
            ManifestMethod {
                name: "startPreview",
                id: 2,
            },
            ManifestMethod {
                name: "stopPreview",
                id: 3,
            },
        ],
    },
    ManifestCapability {
        name: "Storage",
        id: 2,
        methods: &[
            ManifestMethod { name: "set", id: 1 },
            ManifestMethod { name: "get", id: 2 },
            ManifestMethod {
                name: "delete",
                id: 3,
            },
        ],
    },
    ManifestCapability {
        name: "Router",
        id: 3,
        methods: &[ManifestMethod {
            name: "navigate",
            id: 1,
        }],
    },
];

/// Resolves a numeric `(cap_id, method_id)` pair to its wire names.
///
/// Returns `None` when the ids are not part of the MLP manifest — a program
/// that `CALL_CAP`s an unknown id cannot be satisfied by any host and is
/// reported separately as an unknown-capability error.
#[must_use]
pub(crate) fn names_for(cap_id: u32, method_id: u16) -> Option<(&'static str, &'static str)> {
    let cap = CAPABILITIES.iter().find(|c| c.id == cap_id)?;
    let method = cap.methods.iter().find(|m| m.id == method_id)?;
    Some((cap.name, method.name))
}

/// Whether a host's advertised capabilities cover a required
/// `(cap_name, method_name)` pair.
///
/// `advertised` is the `Hello` frame's `capabilities` list
/// `(name, version, features)`. A required method is satisfied when some
/// advertised capability shares its name and lists the method in its features.
#[must_use]
pub(crate) fn is_satisfied(
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

    #[test]
    fn manifest_ids_match_native_registries() {
        // The ids here must equal the native registry tables and
        // stdlib/capabilities.flux.
        assert_eq!(names_for(1, 1), Some(("Camera", "take")));
        assert_eq!(names_for(2, 2), Some(("Storage", "get")));
        assert_eq!(names_for(3, 1), Some(("Router", "navigate")));
        assert_eq!(names_for(9, 9), None);
    }

    #[test]
    fn satisfaction_checks_name_and_feature() {
        let advertised = vec![
            ("Camera".to_owned(), 1, vec!["take".to_owned()]),
            (
                "Storage".to_owned(),
                1,
                vec!["set".to_owned(), "get".to_owned()],
            ),
        ];
        assert!(is_satisfied(&advertised, "Camera", "take"));
        assert!(is_satisfied(&advertised, "Storage", "get"));
        assert!(!is_satisfied(&advertised, "Camera", "stopPreview"));
        assert!(!is_satisfied(&advertised, "Router", "navigate"));
    }
}
