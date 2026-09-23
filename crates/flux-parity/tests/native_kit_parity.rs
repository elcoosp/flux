//! T-504: native_kit_parity — the adapter-drift gate claimed by CHANGELOG.md
//! (FLUX-078).
//!
//! For each component in a fixture table, runs the full parity pipeline
//! (parse → type-check → lower → codegen(Swift, Kotlin) → recognize) and
//! asserts that the dev, Swift, and Kotlin trees are structurally
//! equivalent — including props (now threaded via T-501).

use flux_parity::check_parity;

/// A (component, .flux source) fixture pair.
struct KitFixture {
    name: &'static str,
    source: &'static str,
}

/// Fixture table: each entry exercises a different native adapter component.
const KIT_FIXTURES: &[KitFixture] = &[
    KitFixture {
        name: "Text",
        source: "compo Hello\n  Text(\"Hello\")\n",
    },
    KitFixture {
        name: "Button",
        source: "compo Hello\n  Button(text: \"Save\", onPress: {})\n",
    },
    KitFixture {
        name: "TextInput",
        source: "compo Hello\n  TextInput(value: \"\", onChangeText: {})\n",
    },
    KitFixture {
        name: "Image",
        source: "compo Hello\n  Image(source: \"logo.png\")\n",
    },
    KitFixture {
        name: "Toggle",
        source: "compo Hello\n  Toggle(value: false, onValueChange: {})\n",
    },
    KitFixture {
        name: "ScrollView",
        source: "compo Hello\n  ScrollView {\n    Text(\"content\")\n  }\n",
    },
    KitFixture {
        name: "Column",
        source: "compo Hello\n  Column {\n    Text(\"a\")\n    Text(\"b\")\n  }\n",
    },
    KitFixture {
        name: "Row",
        source: "compo Hello\n  Row {\n    Text(\"a\")\n    Text(\"b\")\n  }\n",
    },
    KitFixture {
        name: "Stack",
        source: "compo Hello\n  Stack {\n    Text(\"a\")\n  }\n",
    },
];

#[test]
fn native_kit_parity_all_components() {
    let mut failures: Vec<String> = Vec::new();
    for fixture in KIT_FIXTURES {
        let report = match check_parity(fixture.source, 1) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: pipeline error: {}", fixture.name, e));
                continue;
            }
        };
        if !report.is_equivalent() {
            failures.push(format!(
                "{}: dev/swift/kotlin trees are DIVERGENT",
                fixture.name
            ));
        }
    }
    if !failures.is_empty() {
        panic!(
            "native_kit_parity failures:\n  - {}",
            failures.join("\n  - ")
        );
    }
}
