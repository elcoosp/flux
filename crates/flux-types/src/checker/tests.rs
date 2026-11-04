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
