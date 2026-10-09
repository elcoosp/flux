use super::*;
mod use_resolution_tests {
    use super::*;
    use crate::{type_check, type_check_with_loader};
    use std::collections::HashMap;

    /// Builds a loader over an in-memory map of module name -> source.
    fn mem_loader(map: HashMap<String, String>) -> ModuleLoader {
        Arc::new(move |name: &str| map.get(name).cloned())
    }

    #[test]
    fn use_theme_brings_exports_into_scope() {
        let mut modules = HashMap::new();
        modules.insert(
            "theme".to_owned(),
            "compo Button(label: String)\n  Text(label)\n".to_owned(),
        );
        let entry = "use theme\n\ncompo Main()\n  Button(label: \"hi\")\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let typed = type_check_with_loader(&ast, Some(mem_loader(modules)));
        assert!(
            typed.is_ok(),
            "use theme should resolve and Button should be in scope: {:?}",
            typed.err()
        );
    }

    #[test]
    fn use_theme_star_is_equivalent() {
        let mut modules = HashMap::new();
        modules.insert(
            "theme".to_owned(),
            "compo Button(label: String)\n  Text(label)\n".to_owned(),
        );
        let entry = "use theme::*\n\ncompo Main()\n  Button(label: \"hi\")\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let typed = type_check_with_loader(&ast, Some(mem_loader(modules)));
        assert!(
            typed.is_ok(),
            "use theme::* should bring exports in: {:?}",
            typed.err()
        );
    }

    #[test]
    fn use_theme_red_brings_only_that_member() {
        let mut modules = HashMap::new();
        modules.insert(
            "theme".to_owned(),
            "compo Button(label: String)\n  Text(label)\n\ncompo Panel()\n  Text(\"x\")\n"
                .to_owned(),
        );
        // `use theme::Button` should expose Button but NOT Panel.
        let entry = "use theme::Button\n\ncompo Main()\n  Button(label: \"hi\")\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        assert!(type_check_with_loader(&ast, Some(mem_loader(modules.clone()))).is_ok());

        let entry_panel = "use theme::Button\n\ncompo Main()\n  Panel()\n";
        let ast_panel = flux_parser::parse(entry_panel, 0, "main").expect("entry parses");
        assert!(
            type_check_with_loader(&ast_panel, Some(mem_loader(modules))).is_err(),
            "use theme::Button must NOT expose Panel"
        );
    }

    #[test]
    fn use_unknown_module_is_actionable_error() {
        let entry = "use missing\n\ncompo Main()\n  Text(\"x\")\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let result = type_check_with_loader(&ast, Some(mem_loader(HashMap::new())));
        assert!(result.is_err(), "unknown module must error");
    }

    #[test]
    fn use_without_loader_is_rejected() {
        // `type_check` (no loader) must reject `use` rather than silently no-op.
        let entry = "use theme\n\ncompo Main()\n  Text(\"x\")\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        assert!(
            type_check(&ast).is_err(),
            "use without a module loader must be rejected with an actionable error"
        );
    }

    #[test]
    fn use_transitive_resolves_nested_modules() {
        let mut modules = HashMap::new();
        modules.insert(
            "base".to_owned(),
            "compo Button(label: String)\n  Text(label)\n".to_owned(),
        );
        modules.insert(
            "theme".to_owned(),
            "use base\n\ncompo Panel()\n  Button(label: \"hi\")\n".to_owned(),
        );
        let entry = "use theme\n\ncompo Main()\n  Panel()\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let typed = type_check_with_loader(&ast, Some(mem_loader(modules)));
        assert!(
            typed.is_ok(),
            "transitive use (theme -> base) should resolve: {:?}",
            typed.err()
        );
    }

    #[test]
    fn use_cycle_is_rejected() {
        let mut modules = HashMap::new();
        modules.insert(
            "a".to_owned(),
            "use b\n\ncompo A()\n  Text(\"x\")\n".to_owned(),
        );
        modules.insert(
            "b".to_owned(),
            "use a\n\ncompo B()\n  Text(\"x\")\n".to_owned(),
        );
        let entry = "use a\n\ncompo Main()\n  Text(\"x\")\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let result = type_check_with_loader(&ast, Some(mem_loader(modules)));
        assert!(result.is_err(), "cyclic use must be rejected");
    }
}

mod nested_use_regression {
    use super::*;
    use crate::type_check_with_loader;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn mem_loader(map: HashMap<String, String>) -> ModuleLoader {
        Arc::new(move |name: &str| map.get(name).cloned())
    }

    /// Regression: a 3-segment `use a::b::c` used to fall through to the
    /// bare-module branch and glob *every* export of `a`, silently leaking
    /// names the user never asked for. It now surfaces a clear error.
    #[test]
    fn three_segment_use_is_rejected_not_globbed() {
        let mut modules = HashMap::new();
        modules.insert(
            "a".to_owned(),
            "compo Kept()\n  Text(\"k\")\n\ncompo Leaked()\n  Text(\"l\")\n".to_owned(),
        );
        let entry = "use a::b::Kept\n\ncompo Main()\n  Kept()\n";
        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let typed = type_check_with_loader(&ast, Some(mem_loader(modules.clone())));
        let err = typed.expect_err("3+ segment use must be rejected");
        assert!(
            format!("{err}").contains("nested module"),
            "error must explain nested-module paths are unsupported: {err}",
        );

        // `use a` and `use a::b` continue to work as before.
        let ok_bare = "use a\n\ncompo Main()\n  Kept()\n";
        let ast_ok = flux_parser::parse(ok_bare, 0, "main").expect("parses");
        assert!(type_check_with_loader(&ast_ok, Some(mem_loader(modules))).is_ok());
    }
}

mod reserved_identifier_regression {
    use crate::type_check;
    use flux_parser::parse;

    /// Regression: a Flux component named `class` (a Swift reserved word) used
    /// to reach the codegen backend, which emitted `struct class: View { ... }`
    /// — invalid Swift that failed the native `swiftc -typecheck` step with an
    /// obscure error. The type checker now rejects any declaration whose name
    /// is reserved in either target language.
    #[test]
    fn swift_keyword_component_name_is_rejected() {
        let src = "compo class\n  Text(\"hi\")\n";
        let ast = parse(src, 0, "main").expect("parses");
        let err = type_check(&ast).expect_err("`class` must be rejected");
        assert!(
            err.message.contains("reserved"),
            "error must mention the reserved-name cause: {}",
            err.message,
        );
        assert!(
            err.hint.as_deref().is_some_and(|h| h.contains("class_")),
            "hint must suggest a rename: {:?}",
            err.hint,
        );
    }

    /// Same rejection for a Kotlin reserved word (`object`).
    #[test]
    fn kotlin_keyword_component_name_is_rejected() {
        let src = "compo object\n  Text(\"hi\")\n";
        let ast = parse(src, 0, "main").expect("parses");
        let err = type_check(&ast).expect_err("`object` must be rejected");
        assert!(err.message.contains("reserved"));
    }

    /// The rejection covers `type` declarations, not just components, since
    /// their name becomes a native `enum`/`struct` identifier.
    #[test]
    fn reserved_type_declaration_name_is_rejected() {
        let src = "type struct = A(Int)\n\ncompo Main\n  Text(\"x\")\n";
        let ast = parse(src, 0, "main").expect("parses");
        let err = type_check(&ast).expect_err("`type struct` must be rejected");
        assert!(err.message.contains("reserved"));
    }

    /// An ordinary component name is unaffected.
    #[test]
    fn ordinary_component_name_is_accepted() {
        let src = "compo Counter\n  Text(\"hi\")\n";
        let ast = parse(src, 0, "main").expect("parses");
        assert!(type_check(&ast).is_ok());
    }

    /// A name that resembles but does not equal a keyword (`Class` vs `class`)
    /// is accepted, matching the target languages' case sensitivity.
    #[test]
    fn case_mismatched_name_is_accepted() {
        let src = "compo Class\n  Text(\"hi\")\n";
        let ast = parse(src, 0, "main").expect("parses");
        assert!(type_check(&ast).is_ok());
    }
}
