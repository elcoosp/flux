//! Codegen source map (FLUX-F17): the sidecar mapping emitted alongside each
//! generated native source, so the release-mode crash reporter can resolve a
//! native stack frame back to its originating `.flux` component.
//!
//! The codegen traversal already recovers every component's AST (via
//! [`crate::Bridge`]), including its [`flux_syntax::Span`]. The source map is
//! simply the (native_symbol, component_name, source_span) tuples recorded at
//! each `emit_component` boundary, serialized to JSON. It is emitted next to
//! each generated file as `<basename>.flux.map.json`; the host bundles it as
//! a resource and resolves crash frames against it.

use serde::{Deserialize, Serialize};

/// One entry in the source map: a native symbol and the `.flux` location it
/// was generated from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapEntry {
    /// The emitted native symbol (e.g. `Hello` for a Swift `struct Hello: View`
    /// or a Kotlin `@Composable fun Hello`). The crash reporter matches the
    /// leading token of a stack frame against this.
    pub native_symbol: String,
    /// The `.flux` component name. Usually identical to `native_symbol` for a
    /// non-generic component; differs for a monomorphisation
    /// (`Counter_Int` → `Counter`).
    pub component_name: String,
    /// The resolved `.flux` source file, as passed to `flux build` (usually a
    /// path relative to the project root).
    pub source_file: String,
    /// 1-based line of the component declaration.
    pub line: u32,
    /// 1-based column of the component declaration.
    pub col: u32,
}

/// A complete source map for one generated file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMap {
    /// Schema version. Bumped on any layout change.
    pub version: u32,
    /// The `.flux` source file this map corresponds to.
    pub source_file: String,
    /// One entry per component emitted into the file.
    pub component_map: Vec<SourceMapEntry>,
}

impl SourceMap {
    /// Current schema version.
    pub const VERSION: u32 = 1;

    /// Builds an empty map for `source_file`.
    #[must_use]
    pub fn new(source_file: impl Into<String>) -> Self {
        Self {
            version: Self::VERSION,
            source_file: source_file.into(),
            component_map: Vec::new(),
        }
    }

    /// Records one emitted component. Duplicates are kept — a generic
    /// component emits multiple native symbols and each is a valid resolution
    /// target; the crash reporter takes the first match.
    pub fn record(&mut self, entry: SourceMapEntry) {
        self.component_map.push(entry);
    }

    /// Serializes to pretty JSON (the on-disk sidecar format).
    ///
    /// # Errors
    ///
    /// Returns the `serde_json` error if serialization fails (only possible on
    /// a schema bug — the struct has no fallible fields).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parses a sidecar from JSON.
    ///
    /// # Errors
    ///
    /// Returns the `serde_json` error when the JSON is malformed or the schema
    /// version is unknown.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let mut map = SourceMap::new("src/Hello.flux");
        map.record(SourceMapEntry {
            native_symbol: "Hello".into(),
            component_name: "Hello".into(),
            source_file: "src/Hello.flux".into(),
            line: 3,
            col: 1,
        });
        let json = map.to_json().expect("serializes");
        let back = SourceMap::from_json(&json).expect("parses");
        assert_eq!(map, back);
    }

    #[test]
    fn empty_map_serializes_with_zero_entries() {
        let map = SourceMap::new("src/Empty.flux");
        let json = map.to_json().expect("serializes");
        assert!(json.contains("\"component_map\": []"), "got {json}");
        assert!(json.contains("\"version\": 1"), "got {json}");
    }
}
