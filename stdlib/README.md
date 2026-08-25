# Flux standard library (`/stdlib`)

The twelve `.flux` files here are the Flux standard library per mlp-spec §18.3
and the Appendix F adapter prop contracts. They are declarations only: the
component bodies are supplied natively by the platform adapters.

| File | Contents |
|---|---|
| `prelude.flux` | auxiliary value types (`Option`, `Alignment`, `Overflow`, `KeyboardType`, `ContentMode`, `Ref`, `Handler`) |
| `traits.flux` | `Numeric`, `Eq`, `Show` |
| `color.flux` | `Color` (`RGB`) + `red`/`green`/`blue`/`black`/`white` |
| `font.flux` | `Font`, `Weight`, `Style` + `body`/`title`/`caption` presets |
| `text.flux`, `button.flux`, `column.flux`, `row.flux`, `text_field.flux` | adapter components (Appendix F.1–F.5) |
| `router.flux` | `Router` + `Screen` |
| `capabilities.flux` | capability declarations (spec §24.1) |
| `platform.flux` | platform tag and query helpers |

## Parse checking (FLUX-015)

`./parse-check.sh` validates every `.flux` file in this directory against the
real parser (`flux_parser::parse`). Run it from anywhere:

```bash
stdlib/parse-check.sh
```

It builds `flux-parser`, compiles `tools/parse_check.rs` against the resulting
rlib, self-tests that `tools/fixtures/invalid.flux` is *rejected* (so a
vacuously-passing harness cannot hide), and then prints one line per stdlib
file plus a failure count. Exit status is non-zero if any file fails, and the
parser's full Rust-style diagnostic is printed for each failure.

The driver is compiled with `rustc` rather than added as a workspace crate on
purpose: `docs/agents-boundaries-contract.md` R2 freezes every build manifest,
so nothing here may edit the root `Cargo.toml`. All files involved live under
`/stdlib`.

The parser crate additionally carries `crates/flux-parser/tests/stdlib.rs`,
which parses the same twelve files inside `cargo nextest run -p flux-parser`.
That is the CI-visible gate; this script is the stdlib-side equivalent that
works without touching the parser crate.

### Grammar gaps G1–G4

`docs/adr/ADR-0037-stdlib-grammar-gaps.md` recorded four constructs the stdlib needed
that the original Appendix B did not spell out. All four are now grammar
productions, reconciled into Appendix B by
`docs/adr/ADR-0035-parser-grammar-extensions.md`:

| Gap | Construct | Production | Used by |
|---|---|---|---|
| G1 | top-level `Name.field = expr` | `const_binding` | `color.flux`, `font.flux` |
| G2 | prop defaults `name: T = expr` | `prop_decl` | `text`, `button`, `column`, `row`, `text_field` |
| G3 | record literal in value position | `record_lit` | available; stdlib prefers positional variants |
| G4 | symbolic operator method names | `fn_name` | `traits.flux` |

No stdlib source change was required to close them, and no gap remains open.
