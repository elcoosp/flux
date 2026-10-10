//! Round-85 regression: `await <expr>` must not type as `Unit`.
//!
//! Before the fix, `ExprKind::Await` had no arm in `infer_inner` and fell
//! into the `_ => Ok(TcType::Unit)` catch-all. A program that used the awaited
//! value as a non-Unit type — `(await Auth.login("u")) + 1` — was rejected
//! with "operator `Add` requires a Numeric type, got `Unit`", a spurious
//! error on a valid program.
//!
//! The fix adds an explicit `Await` arm that infers the inner expression (to
//! surface any errors within it) and returns a fresh inference variable that
//! the surrounding context pins. The language today has no way to declare an
//! async operation's return type, so the fresh variable is the sound choice.

use flux_parser::parse;
use flux_types::type_check;

fn check(src: &str) -> Result<(), String> {
    let ast = parse(src, 0, "t.flux").map_err(|e| format!("parse: {e:?}"))?;
    type_check(&ast)
        .map(|_| ())
        .map_err(|e| e.message.clone())
}

#[test]
fn await_value_can_be_used_as_a_number() {
    // This is the exact shape that was spuriously rejected.
    check("compo C\n  Button(text: \"go\", onPress: { let y = (await Auth.login(\"u\")) + 1 })\n")
        .expect("await result used in arithmetic must type-check");
}

#[test]
fn bare_await_statement_still_works() {
    check("compo C\n  Button(text: \"go\", onPress: { await Auth.login(\"u\") })\n")
        .expect("bare await statement must type-check");
}

#[test]
fn let_await_still_works() {
    check("compo C\n  Button(text: \"go\", onPress: { let x = await Auth.login(\"u\") })\n")
        .expect("`let x = await …` must type-check");
}

#[test]
fn errors_inside_await_are_still_surfaced() {
    // An unbound name inside the awaited call must still be reported — the
    // `Await` arm recurses into the inner expression so the diagnostic is
    // not swallowed.
    let e = check("compo C\n  Button(text: \"go\", onPress: { await NotACapability.foo() })\n")
        .expect_err("unbound capability must error");
    assert!(
        e.contains("NotACapability") || e.contains("unbound"),
        "expected unbound-capability error, got: {e}",
    );
}
