//! Round-21 adversarial tests for `flux_types::type_check`.
//!
//! Targets the `Checker::resolve` fixpoint loop (checker.rs, the 64-pass
//! bound that used to end in `unreachable!`). A *linear* substitution chain
//! is not a cycle: `unify::bind`'s occurs check forbids cycles, but nothing
//! bounds chain *length*. Forward-declared fns hand out fresh return vars
//! (`collect_adts`), so sequential `if` arms unify var→var and build a chain
//! one link per statement; reading the head after 65+ links used to trip the
//! 64-pass `unreachable!` and panic instead of returning a `TypeError`.

use flux_parser::parse;
use flux_types::type_check;

fn check(src: &str) -> Result<flux_types::TypedAST, flux_types::TypeError> {
    let ast = parse(src, 0, "adv.flux").expect("adversarial input must parse");
    type_check(&ast)
}

/// Builds `N` forward-declared zero-arg fns plus a trigger fn whose body is
/// `N-1` sequential `if`s unifying each fn's fresh return var against the
/// next one's, then reads the chain head as the fn body.
fn chain_source(links: usize) -> String {
    let mut src = String::new();
    for i in 1..=links {
        src.push_str(&format!("fn f{i}() {{}}\n"));
    }
    src.push_str("fn trigger(cond: Bool) -> Int {\n");
    // if #k unifies ret(f_k) ~ ret(f_{k+1}): subst[ret_k] = Var(ret_{k+1}).
    for k in 1..links {
        src.push_str(&format!("  if cond {{ f{k}() }} else {{ f{}() }}\n", k + 1));
    }
    // Pin the tail to a concrete type: subst[ret_links] = Int.
    src.push_str(&format!("  if cond {{ f{links}() }} else {{ 1 }}\n"));
    // The head var (ret f1) is only reachable through `links` substitution
    // passes — resolved when the declared `-> Int` is checked.
    src.push_str("  let x = if cond { f1() } else { f2() }\n");
    // Rebuild head link AFTER the tail is pinned? No — the head link is the
    // first if above; `x` aliases it. Put `x` last so resolve sees the whole
    // chain: body type is Var(ret f1).
    src.push_str("  x\n");
    src.push_str("}\n");
    src
}

/// 65+ var→var links exceed the old 64-pass bound. Must return Ok or Err,
/// never panic. (Regression: this panicked with `unreachable!` before the
/// round-21 fix.)
#[test]
fn forward_fn_chain_deeper_than_64_does_not_panic() {
    let src = chain_source(70);
    // Ok or Err are both acceptable; a panic is not.
    let _ = check(&src);
}

/// Sanity: the same construction one link *under* the old bound must keep
/// type-checking cleanly after any fix (guards against over-eager rejection).
#[test]
fn forward_fn_chain_of_10_still_type_checks() {
    let src = chain_source(10);
    let typed = check(&src).expect("10-link chain must type-check");
    let _ = typed;
}

/// Self-referential component: `compo X` whose body instantiates `X`.
/// Must be rejected (or accepted) with an error, never recurse forever.
#[test]
fn self_referential_component_returns_error_not_hang() {
    let src = "compo X\n  X()\n";
    let _ = check(&src);
}

/// Recursive type alias: `type A = | Wrap(A)` (or a self-name alias).
/// Must not hang or panic in ADT collection or unification.
#[test]
fn self_referential_adt_returns_result_not_hang() {
    let src = "type A =\n  | Wrap(A)\nfn f(a: A) -> A { a }\n";
    let _ = check(&src);
}
