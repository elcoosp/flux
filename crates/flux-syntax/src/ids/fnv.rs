const FNV_OFFSET_BASIS: u32 = 0x811C_9DC5;
const FNV_PRIME: u32 = 0x0100_0193;

/// Folds `bytes` into a 32-bit FNV-1a digest.
///
/// `pub` so sibling crates in the workspace (e.g. `flux-ir`'s
/// content-addressing pass, FLUX-074) can derive node IDs with the exact same
/// digest the canonical [`compute_node_id`] uses, keeping every derived ID
/// space in agreement.
#[must_use]
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Stable identity of an IR node, derived from source structure.
pub type NodeId = u32;
/// Index into the host app's closure table.
pub type HandlerId = u32;
/// Index of a cell in the reactive signal graph.
pub type SignalId = u32;
/// Index of an effect owned by a component instance.
pub type EffectId = u32;
/// Interned component name.
pub type ComponentId = u32;
/// Interned string, resolved through a [`crate::StringTable`].
pub type StringId = u32;
/// Identity of a source file.
pub type FileId = u32;
/// Interned type.
pub type TypeId = u32;
/// Index of a prop field within a component's prop layout.
pub type PropIdx = u16;
/// Identity of a live component instance in the host app.
pub type InstanceId = u32;
/// Hash of a `ForEach` item key, used for keyed reconciliation.
pub type Key = u64;
