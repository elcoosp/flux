//! Reference implementation for T-331: ForEach row-id derivation.
//!
//! This is the neutral (compiler) source of truth for the shared algorithm.
//! The vector values printed here are pasted into `tests/isa-vectors/foreach_ids.json`.

/// FNV-1a-32 over a byte slice (D1/D2 normative algorithm).
fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Derives a stable per-row id from the ForEach node id + element index or key.
///
/// Algorithm: `fnv1a(foreach_id as u32 LE || 0x2C || index_or_key as u64 LE) | 0x8000_0000`.
/// The 0x2C byte separates the foreach-id from the row seed so the same index
/// under different ForEach nodes produces distinct ids (D1). The 0x80 marker
/// bit ensures derived ids never collide with real server node ids (D2).
fn derive_row_id(foreach_id: u32, index_or_key: u64) -> u32 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&foreach_id.to_le_bytes());
    bytes.push(0x2C);
    bytes.extend_from_slice(&index_or_key.to_le_bytes());
    let h = fnv1a(&bytes);
    h | 0x8000_0000
}

/// Derives a stable per-child id for a child of an expanded ForEach row.
///
/// Algorithm: `fnv1a(row_id as u64 LE || 0x3F || orig_id as u64 LE) | 0xC000_0000`.
/// The 0x3F byte separates the row id from the original template id. The 0xC0
/// marker distinguishes child ids (0xC0…) from row ids (0x80…), closing the
/// collision space (D2).
fn derive_child_id(row_id: u32, orig_id: u32) -> u32 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(row_id as u64).to_le_bytes());
    bytes.push(0x3F);
    bytes.extend_from_slice(&(orig_id as u64).to_le_bytes());
    let h = fnv1a(&bytes);
    h | 0xC000_0000
}

/// Derives a child id for a *keyed* `WireChild.Splice` item of an expanded
/// ForEach row — uses the splice `key` (u64) instead of the template's
/// `origId`. Same FNV-1a stream + 0x3F byte + 0xC000_0000 marker as
/// [derive_child_id], but seeded by the wire splice key (FLUX-092 / D10).
fn derive_key_child_id(row_id: u32, key: u64) -> u32 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(row_id as u64).to_le_bytes());
    bytes.push(0x3F);
    bytes.extend_from_slice(&key.to_le_bytes());
    let h = fnv1a(&bytes);
    h | 0xC000_0000
}

#[test]
fn foreach_ids_reference_vectors() {
    // Core vector from the roadmap:
    let foreach_id = 20u32;
    let row_index = 1u64;
    let orig_id = 10u32;

    let row_id = derive_row_id(foreach_id, row_index);
    let child_id = derive_child_id(row_id, orig_id);

    println!("foreach_id={foreach_id} row_index={row_index} orig_id={orig_id}");
    println!("row_id=0x{row_id:08X} ({row_id})");
    println!("child_id=0x{child_id:08X} ({child_id})");

    // Additional vectors for the JSON file:
    let v2_row = derive_row_id(20, 0);
    let v2_child = derive_child_id(v2_row, 10);
    println!("v2 row_id=0x{v2_row:08X} child_id=0x{v2_child:08X}");

    let v3_row = derive_row_id(1, 0);
    let v3_child = derive_child_id(v3_row, 1);
    println!("v3 row_id=0x{v3_row:08X} child_id=0x{v3_child:08X}");

    // v4: splice key (non-zero) used as row seed → key_child_id derivation
    let v4_foreach_id = 1u32;
    let v4_key: u64 = 42;
    let v4_row = derive_row_id(v4_foreach_id, v4_key);
    let v4_child = derive_child_id(v4_row, 1);
    let v4_key_child = derive_key_child_id(v4_row, v4_key);
    println!("v4 foreach_id={} key={}", v4_foreach_id, v4_key);
    println!(
        "v4 row_id=0x{v4_row:08X} child_id=0x{v4_child:08X} key_child_id=0x{v4_key_child:08X}"
    );

    // Collision guard: every derived id must carry its marker bit so it
    // never equals a real server node id (all < 0x8000_0000).
    assert!(
        row_id & 0x8000_0000 != 0,
        "row id must have the row marker bit"
    );
    assert_eq!(
        child_id & 0xC000_0000,
        0xC000_0000,
        "child id must have child marker bits"
    );

    // Determinism: same inputs → same outputs.
    assert_eq!(derive_row_id(foreach_id, row_index), row_id);
    assert_eq!(derive_child_id(row_id, orig_id), child_id);

    // Different foreach_id → different row_id (D1).
    let other_row = derive_row_id(21, row_index);
    assert_ne!(row_id, other_row, "distinct foreach ids must diverge");

    // Splice key (u64) used as row seed must differ from index-only.
    let keyed_row = derive_row_id(foreach_id, 0x0100_0000_0000_0001u64);
    assert_ne!(row_id, keyed_row, "splice key must change the row id");

    // Derived ids must never look like real server node ids (< 0x8000_0000).
    assert!(derive_row_id(1, 0) & 0x8000_0000 != 0);
    assert!(derive_child_id(1, 0) & 0xC000_0000 != 0);
}
