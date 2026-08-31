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

pub use derive::{USER_CAP_BASE, derive_capability_id, derive_method_id, is_satisfied};
pub use idl::{CAPABILITY_IDL, CapabilityIdl, MethodIdl};
pub use permission::{PermissionChecker, PermissionKind, required_permission};

mod derive;
mod idl;
mod permission;

#[cfg(test)]
mod tests;
