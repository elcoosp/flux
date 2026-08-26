//! The server-side interner for host-requested strings (brittleness 4a).
//!
//! A host that needs a [`StringId`] for a string it produced at runtime (a
//! formatted label, a user-entered value) used to hash it locally and set the
//! `0x8000_0000` synthetic bit. That fallback silently bypassed interning: two
//! hosts could disagree on an id, and the server could never resolve one back to
//! its text. The `InternString` / `StringInterned` frame pair replaces it, and
//! [`HostStrings`] is the table behind the server's half of that exchange.
//!
//! Ids handed out here live in their own dense region ([`HOST_INTERN_ID_BASE`]
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

/// How many distinct host-interned strings fit below the canonical ceiling.
const HOST_INTERN_CAPACITY: usize =
    (flux_ir_serde::STRING_ID_CANONICAL_CEILING - HOST_INTERN_ID_BASE) as usize;

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
            return HOST_INTERN_ID_BASE + id;
        }
        if self.table.len() >= HOST_INTERN_CAPACITY {
            // Exhausting a one-billion-entry region means the host is interning
            // unbounded generated text. Saturate at the last canonical id rather
            // than overflow into the reserved synthetic space: a repeated id is
            // recoverable, a synthetic one reintroduces the bug 4a removed.
            tracing::warn!(
                interned = self.table.len(),
                "host string region exhausted; reusing the last canonical id"
            );
            return flux_ir_serde::STRING_ID_CANONICAL_CEILING - 1;
        }
        HOST_INTERN_ID_BASE + self.table.intern(text)
    }

    /// Resolves an id previously returned by [`intern`](Self::intern), or `None`
    /// when `id` was never handed out from this region.
    pub(crate) fn resolve(&self, id: StringId) -> Option<&str> {
        self.table.resolve(id.checked_sub(HOST_INTERN_ID_BASE)?)
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
