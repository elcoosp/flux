//! The server-side interner for host-requested strings (brittleness 4a).
//!
//! A host that needs a [`StringId`] for a string it produced at runtime (a
//! formatted label, a user-entered value) used to hash it locally and set the
//! `0x8000_0000` synthetic bit. That fallback silently bypassed interning: two
//! hosts could disagree on an id, and the server could never resolve one back to
//! its text. The `InternString` / `StringInterned` frame pair replaces it, and
//! [`HostStrings`] is the table behind the server's half of that exchange.
//!
//! Ids handed out here live in their own dense regions ([`HOST_INTERN_ID_BASE`]
//! upwards) so a host request can never perturb the ids the compiled tree was
//! serialised with, while still staying below
//! [`flux_ir_serde::STRING_ID_CANONICAL_CEILING`] — the invariant that lets the
//! host drop its synthetic fallback entirely.

use flux_syntax::{StringId, StringTable};

/// First id handed out for a host-requested string.
///
/// The compiler's own arena ids are dense from zero and one per source-level
/// string, so a real project never approaches this base; keeping the two regions
/// disjoint means a host request can never alias an arena id.
const HOST_INTERN_ID_BASE: StringId = 0x4000_0000;

/// First id of the secondary host region, used once the primary is full.
///
/// Splitting the space in two (roadmap Phase 5) buys an early, actionable alarm:
/// the primary boundary is crossed long before ids run out, so the warning fires
/// while allocation is still perfectly unique instead of arriving only when the
/// table is genuinely exhausted and every further id would have to alias.
const HOST_INTERN_SECONDARY_BASE: StringId = 0x7000_0000;

/// How many distinct strings fit in the primary region.
const HOST_INTERN_PRIMARY_CAPACITY: usize =
    (HOST_INTERN_SECONDARY_BASE - HOST_INTERN_ID_BASE) as usize;

/// How many distinct host-interned strings fit below the canonical ceiling,
/// across both regions.
const HOST_INTERN_CAPACITY: usize =
    (flux_ir_serde::STRING_ID_CANONICAL_CEILING - HOST_INTERN_ID_BASE) as usize;

/// Maps a table index onto its wire id, placing overflow in the secondary region.
fn id_for_index(index: StringId) -> StringId {
    if (index as usize) < HOST_INTERN_PRIMARY_CAPACITY {
        HOST_INTERN_ID_BASE + index
    } else {
        HOST_INTERN_SECONDARY_BASE + (index - HOST_INTERN_PRIMARY_CAPACITY as StringId)
    }
}

/// Inverse of [`id_for_index`]; `None` when `id` is outside both host regions.
fn index_for_id(id: StringId) -> Option<StringId> {
    if !(HOST_INTERN_ID_BASE..flux_ir_serde::STRING_ID_CANONICAL_CEILING).contains(&id) {
        return None;
    }
    if id >= HOST_INTERN_SECONDARY_BASE {
        Some(HOST_INTERN_PRIMARY_CAPACITY as StringId + (id - HOST_INTERN_SECONDARY_BASE))
    } else {
        Some(id - HOST_INTERN_ID_BASE)
    }
}

/// Interning table for strings a connected host asked the server to canonicalise.
#[derive(Debug, Default)]
pub(crate) struct HostStrings {
    table: StringTable,
}

impl HostStrings {
    /// Interns `text` and returns its canonical id.
    ///
    /// Interning the same text twice returns the same id, and every returned id
    /// is `< flux_ir_serde::STRING_ID_CANONICAL_CEILING`.
    pub(crate) fn intern(&mut self, text: &str) -> StringId {
        if let Some(id) = self.table.lookup(text) {
            return id_for_index(id);
        }
        if self.table.len() >= HOST_INTERN_CAPACITY {
            // Both regions are gone: the host is interning unbounded generated
            // text. Saturate at the last canonical id rather than overflow into
            // the reserved synthetic space — a repeated id is recoverable, a
            // synthetic one reintroduces the bug 4a removed.
            tracing::warn!(
                interned = self.table.len(),
                "host string regions exhausted; reusing the last canonical id"
            );
            return flux_ir_serde::STRING_ID_CANONICAL_CEILING - 1;
        }
        let index = self.table.intern(text);
        if (index as usize) == HOST_INTERN_PRIMARY_CAPACITY {
            tracing::warn!(
                interned = self.table.len(),
                secondary_base = HOST_INTERN_SECONDARY_BASE,
                "primary host string region full; allocating from the secondary region"
            );
        }
        id_for_index(index)
    }

    /// Resolves an id previously returned by [`intern`](Self::intern), or `None`
    /// when `id` was never handed out from either host region.
    pub(crate) fn resolve(&self, id: StringId) -> Option<&str> {
        self.table.resolve(index_for_id(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir_serde::STRING_ID_CANONICAL_CEILING;

    #[test]
    fn interning_the_same_text_twice_returns_the_same_id() {
        let mut strings = HostStrings::default();
        let first = strings.intern("label");
        assert_eq!(strings.intern("label"), first);
    }

    #[test]
    fn distinct_texts_get_distinct_ids() {
        let mut strings = HostStrings::default();
        assert_ne!(strings.intern("one"), strings.intern("two"));
    }

    #[test]
    fn every_id_is_below_the_canonical_ceiling() {
        let mut strings = HostStrings::default();
        for i in 0..64 {
            let id = strings.intern(&format!("label-{i}"));
            assert!(id < STRING_ID_CANONICAL_CEILING, "id {id} is not canonical");
        }
    }

    #[test]
    fn ids_never_alias_the_compilers_dense_arena_region() {
        let mut strings = HostStrings::default();
        // The arena's own ids start at 0 and stay dense; the host region must
        // begin well above anything a real tree could reach.
        assert!(strings.intern("label") >= HOST_INTERN_ID_BASE);
    }

    #[test]
    fn an_interned_id_resolves_back_to_its_text() {
        let mut strings = HostStrings::default();
        let id = strings.intern("round-trip");
        assert_eq!(strings.resolve(id), Some("round-trip"));
    }

    #[test]
    fn an_id_outside_the_host_region_resolves_to_none() {
        let mut strings = HostStrings::default();
        strings.intern("label");
        assert_eq!(strings.resolve(0), None, "arena ids are not host ids");
        assert_eq!(strings.resolve(HOST_INTERN_ID_BASE + 99), None);
    }

    #[test]
    fn the_two_regions_are_disjoint_and_ordered() {
        // Compile-time: a bad constant must not even build, since every id
        // mapping below depends on this ordering.
        const _: () = assert!(HOST_INTERN_ID_BASE < HOST_INTERN_SECONDARY_BASE);
        const _: () =
            assert!(HOST_INTERN_SECONDARY_BASE < flux_ir_serde::STRING_ID_CANONICAL_CEILING);
        assert_eq!(
            HOST_INTERN_PRIMARY_CAPACITY,
            (HOST_INTERN_SECONDARY_BASE - HOST_INTERN_ID_BASE) as usize
        );
    }

    #[test]
    fn index_mapping_is_a_bijection_across_the_region_boundary() {
        // The boundary is where an off-by-one would silently alias two distinct
        // strings onto one id, so walk straight across it.
        let boundary = HOST_INTERN_PRIMARY_CAPACITY as StringId;
        let mut seen = std::collections::HashSet::new();
        for index in [0, 1, boundary - 2, boundary - 1, boundary, boundary + 1] {
            let id = id_for_index(index);
            assert!(
                id < STRING_ID_CANONICAL_CEILING,
                "id {id} escaped the canonical space"
            );
            assert!(id >= HOST_INTERN_ID_BASE);
            assert_eq!(
                index_for_id(id),
                Some(index),
                "id {id} must map back to index {index}"
            );
            assert!(seen.insert(id), "id {id} was handed out twice");
        }
    }

    #[test]
    fn the_first_secondary_id_is_the_secondary_base() {
        let boundary = HOST_INTERN_PRIMARY_CAPACITY as StringId;
        assert_eq!(id_for_index(boundary), HOST_INTERN_SECONDARY_BASE);
        assert_eq!(
            id_for_index(boundary - 1),
            HOST_INTERN_SECONDARY_BASE - 1,
            "the primary region must fill right up to the secondary base"
        );
    }

    #[test]
    fn ids_in_the_secondary_region_resolve_to_none_when_unallocated() {
        let strings = HostStrings::default();
        assert_eq!(strings.resolve(HOST_INTERN_SECONDARY_BASE), None);
    }

    #[test]
    fn an_id_at_or_above_the_ceiling_is_never_a_host_id() {
        // A synthetic host fallback id must never be mistaken for one of ours.
        assert_eq!(index_for_id(STRING_ID_CANONICAL_CEILING), None);
        assert_eq!(index_for_id(0xFFFF_FFFF), None);
    }

    #[test]
    fn the_empty_string_interns_like_any_other() {
        let mut strings = HostStrings::default();
        let id = strings.intern("");
        assert!(id < STRING_ID_CANONICAL_CEILING);
        assert_eq!(strings.resolve(id), Some(""));
    }

    #[test]
    fn unicode_text_round_trips() {
        let mut strings = HostStrings::default();
        let text = "日本語 — émoji 🎛";
        let id = strings.intern(text);
        assert_eq!(strings.resolve(id), Some(text));
        assert_eq!(strings.intern(text), id);
    }
}
