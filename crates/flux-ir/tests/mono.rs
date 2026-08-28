//! Monomorphisation of generic components through lowering (roadmap Phase 1).
//!
//! `Counter[Int]` and `Counter[Float]` must become two *distinct* components in
//! the lowered IR so the release backends emit two separate native types and
//! the runtime never loses the type argument.

use flux_ir::lower;
use flux_parser::parse;
use flux_types::type_check;

/// Two `Counter` call sites with `Int` and `Float` arguments.
const GENERIC_SRC: &str = "trait Numeric[T] {\n  fn zero() -> T\n  fn one() -> T\n}\n\ncompo Counter[T: Numeric](initial: T)\n  state count: T = initial\n\n\ncompo IntCase\n  Counter(initial: 0)\n\n\ncompo FloatCase\n  Counter(initial: 0.0)\n\n";

fn lowered(src: &str) -> flux_ir::LoweredIr {
    let ast = parse(src, 0, "mono.flux").expect("parses");
    let typed = type_check(&ast).expect("type-checks");
    lower(&ast, &typed).expect("lowers")
}

#[test]
fn generic_instantiations_are_recorded_on_lowered_ir() {
    let ir = lowered(GENERIC_SRC);
    assert!(
        ir.requires_monomorph(),
        "a program with generic call sites must require monomorphisation"
    );
    let mut names = ir.specialised_names();
    names.sort_unstable();
    assert_eq!(names, vec!["Counter_Float", "Counter_Int"]);
}

#[test]
fn each_instantiation_gets_its_own_component_id() {
    let ir = lowered(GENERIC_SRC);
    let int_id = ir
        .component_names
        .iter()
        .find(|(_, n)| n == "Counter_Int")
        .map(|(id, _)| *id)
        .expect("Counter_Int must be interned");
    let float_id = ir
        .component_names
        .iter()
        .find(|(_, n)| n == "Counter_Float")
        .map(|(id, _)| *id)
        .expect("Counter_Float must be interned");
    assert_ne!(
        int_id, float_id,
        "distinct instantiations must not share a ComponentId"
    );
    // The generic *template* declaration keeps its own id (it is still a
    // declaration in the tree), but no call site may be interned under it.
    let template_id = ir
        .component_names
        .iter()
        .find(|(_, n)| n == "Counter")
        .map(|(id, _)| *id)
        .expect("the generic template declaration is still interned");
    assert_ne!(template_id, int_id);
    assert_ne!(template_id, float_id);
}

#[test]
fn non_generic_program_requires_no_monomorph() {
    let ir = lowered("compo Hello\n  Button(text: \"tap\")\n");
    assert!(!ir.requires_monomorph());
    assert!(ir.monomorphizations.is_empty());
}
