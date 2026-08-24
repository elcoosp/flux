//! The interning table for all source-derived strings (Appendix C §C.3).

use ahash::AHashMap;

use crate::ids::StringId;

/// Bidirectional interning table mapping strings to compact [`StringId`]s.
///
/// The wire protocol ships only IDs plus a delta of newly interned strings, so
/// the table is the authority both codegen and the host app resolve against.
///
/// # Examples
///
/// ```
/// use flux_syntax::StringTable;
///
/// let mut table = StringTable::new();
/// let id = table.intern("Column");
/// assert_eq!(table.intern("Column"), id);
/// assert_eq!(table.resolve(id), Some("Column"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct StringTable {
    strings: Vec<String>,
    lookup: AHashMap<String, StringId>,
}

impl StringTable {
    /// Creates an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty table with room for `capacity` distinct strings.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            strings: Vec::with_capacity(capacity),
            lookup: AHashMap::with_capacity(capacity),
        }
    }

    /// Interns `text`, returning its existing ID when already present.
    ///
    /// IDs are assigned densely from zero in first-insertion order, which the
    /// wire protocol relies on for delta encoding.
    pub fn intern(&mut self, text: &str) -> StringId {
        if let Some(id) = self.lookup.get(text) {
            return *id;
        }
        let id = self.strings.len() as StringId;
        self.strings.push(text.to_owned());
        self.lookup.insert(text.to_owned(), id);
        id
    }

    /// Resolves `id` to its text, or `None` when `id` was never interned.
    #[must_use]
    pub fn resolve(&self, id: StringId) -> Option<&str> {
        self.strings.get(id as usize).map(String::as_str)
    }

    /// Returns the ID of `text` without interning it.
    #[must_use]
    pub fn lookup(&self, text: &str) -> Option<StringId> {
        self.lookup.get(text).copied()
    }

    /// Returns the number of distinct interned strings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns `true` when nothing has been interned yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Iterates over every interned string paired with its ID, in ID order.
    pub fn iter(&self) -> impl Iterator<Item = (StringId, &str)> {
        self.strings
            .iter()
            .enumerate()
            .map(|(index, text)| (index as StringId, text.as_str()))
    }

    /// Returns the strings interned at or after `id`, in ID order.
    ///
    /// This is the delta the dev server ships when the host app already knows
    /// the first `id` entries (Appendix D §D.9).
    #[must_use]
    pub fn delta_from(&self, id: StringId) -> &[String] {
        let start = (id as usize).min(self.strings.len());
        &self.strings[start..]
    }
}
