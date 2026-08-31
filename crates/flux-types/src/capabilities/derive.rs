/// The capability-id band reserved for user-authored escape-hatch wrappers
/// (FLUX-046). Framework capabilities occupy 1..=11 (stable, hand-assigned);
/// any `derive_capability_id` result lands in `[USER_CAP_BASE, USER_CAP_BASE +
/// 0x0FFF]` so a user module can never collide with a framework id.
pub const USER_CAP_BASE: u32 = 0x1000;

/// Deterministic capability id for a user-authored native-module wrapper
/// (FLUX-046).
///
/// Mirrors the framework's "ids are derived, never hand-assigned" rule
/// (AGENTS.md §3.4): a wrapper's `cap_id` is `FNV-1a(name)` masked into the
/// reserved [`USER_CAP_BASE`] band, so the server and both hosts agree on the
/// exact `(cap_id, method_id)` bytes that travel on the wire. A `.flux` source
/// and the host registry must compute the same id.
#[must_use]
pub fn derive_capability_id(name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in name.as_bytes() {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    USER_CAP_BASE + (hash & 0x0FFF)
}

/// Deterministic method id for a user-authored native-module wrapper method
/// (FLUX-046). Same FNV-1a scheme as [`derive_capability_id`], masked into the
/// same reserved band so user method ids never collide with framework ids.
#[must_use]
pub fn derive_method_id(name: &str) -> u16 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in name.as_bytes() {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (USER_CAP_BASE + (hash & 0x0FFF)) as u16
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
