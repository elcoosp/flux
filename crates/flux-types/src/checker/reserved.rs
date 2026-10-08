//! Reserved-identifier rejection for codegen targets (FLUX-020 / FLUX-021).
//!
//! The Swift and Kotlin codegen backends emit a Flux component / state / prop
//! name verbatim as a native identifier — `struct {name}: View`, `@State var
//! {name}`, etc. A Flux author may choose any non-Flux-keyword identifier, so
//! a component named `class` (Swift reserved) or `object` (Kotlin reserved)
//! previously produced source that would not compile at the native step.
//!
//! Since Flux has no target-language-specific identifier syntax, the only
//! honest way to keep both backends sound is to reject any declaration name
//! that collides with a Swift or Kotlin reserved word at *type-check* time,
//! before the pipeline lowers or emits anything. The error is target-agnostic
//! (a name reserved in either language is rejected), because the same Flux
//! program is expected to compile to both targets.

use flux_syntax::Span;

/// Reserved words in Swift 5.x and Kotlin 2.x, unioned.
///
/// Kept as a sorted slice so the membership check is a binary search and the
/// list is visible in one place. Extend on a new target-language release; do
/// not add words that are only "soft" keywords (Swift `get`, `set`, `willSet`,
/// `didSet`, `_`, `some`, `any`, `async`, `await`, `throws`, `rethrows`; Kotlin
/// `by`, `catch`, `constructor`, `delegate`, `dynamic`, `field`, `file`,
/// `finally`, `get`, `import`, `init`, `param`, `property`, `receiver`, `set`,
/// `setparam`, `where`, `actual`, `abstract`, `annotation`, `companion`,
/// `const`, `crossinline`, `data`, `enum`, `expect`, `external`, `final`,
/// `infix`, `inline`, `inner`, `internal`, `lateinit`, `noinline`, `open`,
/// `operator`, `out`, `override`, `private`, `protected`, `public`,
/// `reified`, `sealed`, `suspend`, `tailrec`, `vararg`, `value`) — those are
/// legal identifiers in their own language and a Flux name that matches one is
/// not actually invalid output.
const RESERVED: &[&str] = &[
    "Any", "Self", "as", "associatedtype", "break", "case", "catch", "class",
    "continue", "default", "defer", "deinit", "do", "else", "enum",
    "extension", "fallthrough", "false", "fileprivate", "for", "fun", "func",
    "guard", "if", "import", "in", "init", "inout", "interface", "internal",
    "is", "let", "nil", "null", "object", "open", "operator", "package",
    "private", "protocol", "public", "repeat", "rethrows", "return", "self",
    "static", "struct", "subscript", "super", "switch", "this", "throw",
    "throws", "true", "try", "typealias", "typeof", "val", "var", "when",
    "where", "while",
];

/// Returns `true` when `name` is reserved in Swift or Kotlin and therefore
/// cannot be emitted verbatim as a native identifier by either backend.
///
/// Matching is exact and case-sensitive: `class` is reserved, `Class` is not,
/// which mirrors the two target languages' own case-sensitivity.
#[must_use]
pub(super) fn is_reserved(name: &str) -> bool {
    RESERVED.binary_search(&name).is_ok()
}

/// A declaration's name and the span of that name, for a diagnostic. Every
/// top-level declaration kind that ends up as a native identifier is covered.
pub(super) fn decl_name(decl: &flux_parser::Decl) -> Option<(&str, Span)> {
    use flux_parser::Decl;
    match decl {
        Decl::Component(c) => Some((c.name.name.as_str(), c.name.span)),
        Decl::Fn(f) => Some((f.name.text.as_str(), f.name.span)),
        Decl::Record(r) => Some((r.name.name.as_str(), r.name.span)),
        Decl::Type(t) => Some((t.name.name.as_str(), t.name.span)),
        Decl::Trait(t) => Some((t.name.name.as_str(), t.name.span)),
        Decl::Capability(c) => Some((c.name.name.as_str(), c.name.span)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swift_reserved_words_are_rejected() {
        for w in ["class", "struct", "func", "let", "var", "Self", "self", "nil", "true", "false"] {
            assert!(is_reserved(w), "Swift keyword `{w}` must be rejected");
        }
    }

    #[test]
    fn kotlin_reserved_words_are_rejected() {
        for w in ["fun", "object", "val", "when", "null", "package", "interface"] {
            assert!(is_reserved(w), "Kotlin keyword `{w}` must be rejected");
        }
    }

    #[test]
    fn ordinary_names_are_accepted() {
        for w in ["Counter", "Button", "count", "items", "onPress", "class_", "Class"] {
            assert!(!is_reserved(w), "ordinary identifier `{w}` must be allowed");
        }
    }

    #[test]
    fn list_is_sorted() {
        // `is_reserved` uses binary search; enforce the invariant.
        let mut sorted = RESERVED.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, RESERVED.to_vec(), "RESERVED must be sorted");
    }
}
