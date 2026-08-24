//! Every `.flux` file in `/stdlib` must parse.
//!
//! The stdlib exercises the constructs recorded as gaps G1–G4 in
//! `/docs/adr/stdlib-grammar-gaps.md`; parsing all twelve files is the
//! parser-side evidence those gaps are closed.

use std::fs;
use std::path::{Path, PathBuf};

use flux_parser::parse;

fn stdlib_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("stdlib")
}

fn flux_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(stdlib_dir())
        .expect("the stdlib directory exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "flux"))
        .collect();
    files.sort();
    files
}

#[test]
fn every_stdlib_file_parses() {
    let files = flux_files();
    assert!(!files.is_empty(), "expected stdlib .flux sources");
    for (index, path) in files.iter().enumerate() {
        let source = fs::read_to_string(path).expect("stdlib file is readable");
        let display = path.display().to_string();
        if let Err(error) = parse(&source, index as u32, &display) {
            panic!("{}", error.render());
        }
    }
}

#[test]
fn g1_associated_constant_bindings_parse() {
    let source = fs::read_to_string(stdlib_dir().join("color.flux")).expect("color.flux");
    let ast = parse(&source, 0, "color.flux").expect("color.flux parses");
    let constants: Vec<&flux_parser::ConstBinding> = ast
        .decls
        .iter()
        .filter_map(|decl| match decl {
            flux_parser::Decl::Const(binding) => Some(binding),
            _ => None,
        })
        .collect();
    assert_eq!(constants.len(), 5);
    assert_eq!(constants[0].path[0].name, "Color");
    assert_eq!(constants[0].path[1].name, "red");
}

#[test]
fn g2_prop_defaults_parse() {
    let source = fs::read_to_string(stdlib_dir().join("text.flux")).expect("text.flux");
    let ast = parse(&source, 0, "text.flux").expect("text.flux parses");
    let flux_parser::Decl::Component(text) = &ast.decls[0] else {
        panic!("expected the Text component");
    };
    assert_eq!(text.props[0].name.name, "text");
    assert!(text.props[0].default.is_none(), "`text` is required");
    assert!(
        text.props[1..].iter().all(|prop| prop.default.is_some()),
        "every optional prop carries its `= None` default"
    );
}

#[test]
fn g4_operator_trait_methods_parse() {
    let source = fs::read_to_string(stdlib_dir().join("traits.flux")).expect("traits.flux");
    let ast = parse(&source, 0, "traits.flux").expect("traits.flux parses");
    let operators: Vec<String> = ast
        .decls
        .iter()
        .filter_map(|decl| match decl {
            flux_parser::Decl::Trait(decl) => Some(decl),
            _ => None,
        })
        .flat_map(|decl| decl.methods.iter())
        .filter(|method| method.name.is_operator)
        .map(|method| method.name.text.clone())
        .collect();
    assert_eq!(operators, vec!["+", "-", "==", "!="]);
}
