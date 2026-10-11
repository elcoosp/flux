//! FLUX-F42 regression: the sample entry the `flux init` scaffold writes
//! must parse and compile, and must not use the Kotlin-style `${expr}`
//! interpolation that Flux does not recognize.

use flux_cli::SAMPLE_ENTRY;

#[test]
fn sample_entry_parses() {
    flux_parser::parse(SAMPLE_ENTRY, 0, "main.flux")
        .expect("the scaffolded sample must parse");
}

#[test]
fn sample_entry_has_no_kotlin_interpolation_syntax() {
    // Flux interpolation is `{expr}`; `${expr}` (Kotlin) renders with a
    // literal `$` and is never the author's intent. The scaffold is the
    // first Flux program every user sees — it must not carry the bug.
    assert!(
        !SAMPLE_ENTRY.contains("${"),
        "the scaffolded sample must not use Kotlin-style `${{expr}}` interpolation: \
         Flux spells it `{{expr}}`. Found in:\n{SAMPLE_ENTRY}"
    );
}

#[test]
fn sample_entry_has_flux_interpolation() {
    // Positive assertion: the sample *does* interpolate `count`, so the
    // scaffold demonstrates the feature.
    assert!(
        SAMPLE_ENTRY.contains("{count}"),
        "the scaffolded sample should interpolate `count` with Flux syntax `{{count}}`"
    );
}
