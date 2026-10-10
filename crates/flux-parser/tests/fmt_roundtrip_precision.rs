//! Round-78 regression: `flux fmt` must not lose f64 precision.
//!
//! `write_float` used `format!("{val:.15}")`, which truncated to 15 decimal
//! places and lost precision for any f64 needing more than 15 significant
//! digits to round-trip:
//!
//! * `0.30000000000000004` was printed as `0.3` (different f64).
//! * The smallest positive f64 (`5e-324`) was printed as `0.0` (total loss).
//!
//! The fix uses Rust's default `Display` for f64 (the shortest decimal form
//! that round-trips to the same bits), with a `.0` appended when the value is
//! whole so the lexer still sees a Float.

use flux_parser::{BlockItem, Decl, ExprKind};

#[test]
fn float_literals_round_trip_losslessly() {
    let sources = [
        "0.30000000000000004",
        "0.12345678901234568",
        "3.141592653589793",
        "1.7976931348623157e308",
        "5e-324",
        "0.1",
        "1.5",
    ];
    for lit in sources {
        let src = format!("compo T\n  state x: Float = {lit}\n");
        let formatted = flux_parser::format_str(&src, "t.flux")
            .unwrap_or_else(|e| panic!("format failed for {lit}: {e:?}"));
        let reparsed = flux_parser::parse(&formatted, 0, "t.flux").unwrap_or_else(|e| {
            panic!("reparse failed for {lit}: {e:?}\nformatted: {formatted}")
        });
        for decl in &reparsed.decls {
            if let Decl::Component(c) = decl {
                for item in &c.body.items {
                    if let BlockItem::State(s) = item {
                        match &s.init.kind {
                            ExprKind::Float(v) => {
                                let orig: f64 = lit.parse().expect("literal parses");
                                assert_eq!(
                                    v.to_bits(),
                                    orig.to_bits(),
                                    "fmt lost precision: {lit} -> {formatted:?} -> {v}",
                                );
                            }
                            other => panic!("expected Float, got {other:?}"),
                        }
                    }
                }
            }
        }
    }
}
