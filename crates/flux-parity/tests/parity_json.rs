//! Phase 4 (roadmap) deterministic parity: the dev-path AST tree and the
//! release-path lowered-IR tree must serialize to identical JSON.
//!
//! This is the formatting-agnostic verdict the plan's Option A calls for: instead
//! of re-parsing the generated Swift/Kotlin source through a tokenizer (which is
//! correct but brittle to cosmetic backend drift), both sides reduce to the same
//! [`ViewNode`] vocabulary and we compare their `serde_json` renderings. No Swift
//! `VStack` vs Kotlin `Column`, no `\(x)` vs `${x}` can ever produce a false
//! divergence. Every Appendix B.3 example is proven equivalent this way.

use flux_codegen_core::{Bridge, view_tree};
use flux_parity::{all_examples, compile, from_ast};
use serde_json::to_string as json;

/// Asserts that the dev-path `from_ast` tree and the release-path `view_tree`
/// tree serialize to byte-identical JSON for every B.3 example.
#[test]
fn json_parity_all_examples() {
    for (idx, (name, source)) in all_examples().iter().enumerate() {
        let file_id = idx as u32;
        let (ast, _typed, lowered) = compile(source, file_id).expect("pipeline");
        let bridge = Bridge::build(&ast);
        let dev = from_ast(&ast);
        let release = view_tree(&lowered, &bridge);

        let dev_json = json(&dev).expect("dev json");
        let release_json = json(&release).expect("release json");
        assert_eq!(
            dev_json, release_json,
            "JSON parity divergence for {name}:\nDEV    = {dev_json}\nRELEASE= {release_json}"
        );
    }
}
