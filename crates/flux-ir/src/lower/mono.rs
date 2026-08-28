//! Monomorphisation bookkeeping for generic component instantiations
//! (spec §18.2/§20.3, roadmap Phase 1).
//!
//! The type checker records every generic instantiation it resolves in
//! [`flux_types::TypedAST::instantiations`], in the order it walked the source.
//! Lowering walks declarations and expressions in that same order, so a
//! per-name cursor over that list maps each *call site* of a generic component
//! to its resolved type arguments without re-running inference.
//!
//! Each mapped call site interns a **specialised** component name
//! (`Counter[Int]` → `Counter_Int`) so the release backends can emit one native
//! type per instantiation and the wire's `component_names` table stays
//! unambiguous.

use std::collections::HashMap;

use flux_types::{GenericInstantiation, TypedAST};

/// One resolved generic instantiation, ready for codegen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monomorphization {
    /// The generic component's source name, e.g. `Counter`.
    pub name: String,
    /// The specialised (mangled) name, e.g. `Counter_Int`.
    pub mangled: String,
    /// Rendered concrete type arguments, e.g. `["Int"]`.
    pub args: Vec<String>,
}

/// Mangles `name` with its resolved type `args` into a native-safe identifier.
///
/// Every character that is not ASCII alphanumeric becomes `_`, so nested
/// arguments (`List[Int]`) stay legal Swift/Kotlin identifiers
/// (`Holder_List_Int_`).
///
/// # Examples
///
/// ```
/// use flux_ir::lower::mangle_specialised;
///
/// assert_eq!(mangle_specialised("Counter", &["Int".to_owned()]), "Counter_Int");
/// ```
#[must_use]
pub fn mangle_specialised(name: &str, args: &[String]) -> String {
    if args.is_empty() {
        return name.to_owned();
    }
    let mut out = String::with_capacity(name.len() + args.len() * 8);
    out.push_str(name);
    for arg in args {
        out.push('_');
        for ch in arg.chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
            } else {
                out.push('_');
            }
        }
    }
    out
}

/// Per-name cursor over `TypedAST::instantiations`, resolving each generic
/// call site in source order to its specialised name.
#[derive(Debug, Default)]
pub(crate) struct MonoTable {
    /// Instantiations grouped by generic name, in checker (source) order.
    by_name: HashMap<String, Vec<Monomorphization>>,
    /// How many call sites of each name have already been resolved.
    cursor: HashMap<String, usize>,
    /// Every instantiation actually reached by lowering, in resolution order.
    resolved: Vec<Monomorphization>,
}

impl MonoTable {
    /// Builds the table from a type-checked program.
    pub(crate) fn new(typed: &TypedAST) -> Self {
        let mut by_name: HashMap<String, Vec<Monomorphization>> = HashMap::new();
        for inst in &typed.instantiations {
            let mono = to_mono(inst);
            by_name.entry(mono.name.clone()).or_default().push(mono);
        }
        Self {
            by_name,
            cursor: HashMap::new(),
            resolved: Vec::new(),
        }
    }

    /// Resolves the next call site of `name`, returning its specialised name
    /// when `name` is a generic component with recorded instantiations.
    ///
    /// Non-generic calls (`Text`, `Column`, a monomorphic component) return
    /// `None` and keep their source name.
    pub(crate) fn next_specialised(&mut self, name: &str) -> Option<String> {
        let list = self.by_name.get(name)?;
        let cursor = self.cursor.entry(name.to_owned()).or_insert(0);
        // A generic component may be instantiated fewer times than it is
        // called when the checker could not resolve every argument; reuse the
        // last recorded instantiation rather than losing specialisation.
        let mono = list.get(*cursor).or_else(|| list.last())?;
        *cursor += 1;
        if !self.resolved.contains(mono) {
            self.resolved.push(mono.clone());
        }
        Some(mono.mangled.clone())
    }

    /// Every instantiation lowering actually specialised, in resolution order.
    pub(crate) fn into_resolved(self) -> Vec<Monomorphization> {
        self.resolved
    }
}

/// Converts a checker instantiation into its codegen-facing form.
fn to_mono(inst: &GenericInstantiation) -> Monomorphization {
    let args: Vec<String> = inst.generic_args.iter().map(ToString::to_string).collect();
    Monomorphization {
        mangled: mangle_specialised(&inst.name, &args),
        name: inst.name.clone(),
        args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mangles_scalar_argument() {
        assert_eq!(
            mangle_specialised("Counter", &["Int".to_owned()]),
            "Counter_Int"
        );
    }

    #[test]
    fn mangles_nested_argument_to_identifier_safe_form() {
        assert_eq!(
            mangle_specialised("Holder", &["List[Int]".to_owned()]),
            "Holder_List_Int_"
        );
    }

    #[test]
    fn no_arguments_keeps_source_name() {
        assert_eq!(mangle_specialised("Counter", &[]), "Counter");
    }
}
