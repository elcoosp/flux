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
/// Camera, cap 2 = Storage, cap 3 = Router). Sync vs async is a binding
/// detail: sync methods return immediately; async methods (most platform
/// calls — camera, permissions, network) resolve through the VM's await
/// machinery (ADR-0044 / ADR-0045) and return a `Result` on failure.
pub const CAPABILITY_IDL: &[CapabilityIdl] = &[
    CapabilityIdl {
        name: "Camera",
        id: 1,
        methods: &[
            MethodIdl {
                name: "take",
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
            MethodIdl { name: "set", id: 1 },
            MethodIdl { name: "get", id: 2 },
            MethodIdl {
                name: "delete",
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
];

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
