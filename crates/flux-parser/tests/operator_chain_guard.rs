//! Round-13: every operator-precedence loop in the recursive-descent parser
//! builds a *left-nested* tree one level per operator. An unbounded chain
//! therefore produces an N-deep `Expr` whose recursive `Drop` overflows the
//! parse thread's stack. Each loop now bounds the chain at `MAX_PARSE_DEPTH`.
//!
//! This test drives every such chain length well past the bound on a small
//! (512 KiB) worker stack. Before the fix, `1+1+1+...` × 5 000 overflowed here.

use flux_parser::parse;

const SMALL_STACK: usize = 512 * 1024;

fn probe_on_small_stack(name: &str, src: String) -> String {
    let h = std::thread::Builder::new()
        .name(format!("probe-{name}"))
        .stack_size(SMALL_STACK)
        .spawn(move || match parse(&src, 0, "p.flux") {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("ERR: {}", e.message),
        })
        .expect("spawn");
    h.join()
        .unwrap_or_else(|_| panic!("[{name}] thread aborted (stack overflow)"))
}

#[test]
fn long_add_chain_is_rejected() {
    let mut s = String::from("compo X\n  Button(disabled: 1");
    for _ in 0..5000 {
        s.push_str("+1");
    }
    s.push_str(")\n");
    let out = probe_on_small_stack("add", s);
    assert!(
        out.contains("chain too long"),
        "expected chain-length error, got: {out}",
    );
}

#[test]
fn long_or_chain_is_rejected() {
    let mut s = String::from("compo X\n  Button(disabled: false");
    for _ in 0..5000 {
        s.push_str(" || false");
    }
    s.push_str(")\n");
    let out = probe_on_small_stack("or", s);
    assert!(out.contains("chain too long"), "got: {out}");
}

#[test]
fn long_and_chain_is_rejected() {
    let mut s = String::from("compo X\n  Button(disabled: true");
    for _ in 0..5000 {
        s.push_str(" && true");
    }
    s.push_str(")\n");
    let out = probe_on_small_stack("and", s);
    assert!(out.contains("chain too long"), "got: {out}");
}

#[test]
fn long_not_chain_is_rejected() {
    let mut s = String::from("compo X\n  Button(disabled: ");
    for _ in 0..5000 {
        s.push('!');
    }
    s.push_str("false)\n");
    let out = probe_on_small_stack("not", s);
    assert!(
        out.contains("nesting too deep") || out.contains("too long"),
        "got: {out}",
    );
}

#[test]
fn moderate_chains_parse() {
    // 50 terms — far under the cap — must still parse.
    let mut s = String::from("compo X\n  Button(disabled: 1");
    for _ in 0..50 {
        s.push_str("+1");
    }
    s.push_str(")\n");
    let out = probe_on_small_stack("moderate", s);
    assert_eq!(out, "OK", "50-term chain must parse, got: {out}");
}
