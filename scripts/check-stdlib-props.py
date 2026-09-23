#!/usr/bin/env python3
"""
Stdlib↔kit prop contract auditor (T-503).

Scans `stdlib/*.flux` for component prop declarations, scans the Kotlin and
Swift adapter kits for which props each reads (not merely defines in PropsIndex),
then reports any drift:

  * declared but read by NEITHER kit
  * declared and read by only ONE kit
  * read by a kit but not declared

Usage:
  python3 scripts/check-stdlib-props.py
Exit 1 on any violation.

Limitation: the Kotlin PropsIndex maps names via a prefix (e.g.
`TEXT_INPUT_KEYBOARD_TYPE`), so component name is inferred from the prefix.
The Swift prop reads are component-specific via the adapter class.
"""

import pathlib
import re
import sys
from collections import defaultdict

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def parse_stdlib_props(stdlib_dir: pathlib.Path) -> dict[str, set[str]]:
    """Parse `compo Name(prop: Type, ...)` declarations from stdlib .flux files.

    Limitation: arg lists with default values containing parens
    (e.g. `= fn() {}`) defeat a simple `([^)]*)` capture, so we use a
    brace-balanced scan instead of a single regex.
    """
    props: dict[str, set[str]] = defaultdict(set)
    for path in sorted(stdlib_dir.glob("*.flux")):
        text = path.read_text()
        # Find `compo Name(` and then scan to the matching close paren
        # (handling nested parens in default values like `fn() {}`).
        for m in re.finditer(r"compo\s+(\w+)\s*\(", text):
            comp_name = m.group(1)
            start = m.end()
            depth = 1
            i = start
            while i < len(text) and depth > 0:
                if text[i] == "(":
                    depth += 1
                elif text[i] == ")":
                    depth -= 1
                i += 1
            args_str = text[start : i - 1]  # exclude the closing )
            for arg in args_str.split(","):
                arg = arg.strip()
                if not arg or arg.startswith("//"):
                    continue
                pm = re.match(r"(\w+)\s*:", arg)
                if pm:
                    props[comp_name].add(pm.group(1))
    return props


# Maps PropsIndex constant prefix → component name.
KOTLIN_COMPONENT_PREFIXES = [
    ("TEXT_INPUT", "TextInput"),
    ("TEXT_AREA", "TextArea"),
    ("DATE_PICKER", "DatePicker"),
    ("SCROLL", "ScrollView"),
    ("SAFEAREA", "SafeArea"),
    ("OVERLAY", "Dialog"),
    ("WEB_HOST", "WebHost"),
    ("ON_DISMISS", "Dialog"),
    ("TEXT", "Text"),
    ("BUTTON", "Button"),
    ("IMAGE", "Image"),
    ("TOGGLE", "Toggle"),
    ("CHECKBOX", "Checkbox"),
    ("SLIDER", "Slider"),
    ("SWITCH", "Switch"),
    ("COLUMN", "Column"),
    ("ROW", "Row"),
    ("STACK", "Stack"),
    ("GRID", "Grid"),
    ("SPACER", "Spacer"),
    ("PICKER", "Picker"),
    ("FONT", "Font"),
    ("COLOR", "Color"),
    ("GESTURE", "Gesture"),
    ("ANIMATE", "Animate"),
]


def kotlin_const_to_comp(const_name: str) -> str:
    """Infer component name from a PropsIndex constant name."""
    for prefix, comp in KOTLIN_COMPONENT_PREFIXES:
        if const_name.startswith(prefix + "_"):
            return comp
    return "Unknown"


def parse_kotlin_reads(kotlin_dir: pathlib.Path) -> dict[str, set[str]]:
    """Scan Kotlin adapter files for props.<accessor>(PropsIndex.X) patterns.

    Only props actually READ in an adapter file are counted — merely
    defining a PropsIndex constant does not count as a read.
    """
    reads: dict[str, set[str]] = defaultdict(set)
    props_index_path = kotlin_dir / "src/main/kotlin/dev/flux/ui/PropsIndex.kt"
    if not props_index_path.exists():
        return reads
    # Build const_name → prop_name map from PropsIndex
    idx_text = props_index_path.read_text()
    idx_pattern = re.compile(
        r'val\s+(\w+)\s*:\s*UShort\s*=\s*propIndexForName\("(\w+)"\)'
    )
    const_to_prop: dict[str, str] = {}
    for m in idx_pattern.finditer(idx_text):
        const_to_prop[m.group(1)] = m.group(2)

    # Scan adapter files for actual reads
    ui_dir = kotlin_dir / "src/main/kotlin/dev/flux/ui"
    file_to_comp = {
        "TextAdapter.kt": "Text",
        "TextInputAdapter.kt": "TextInput",
        "TextAreaAdapter.kt": "TextArea",
        "ButtonAdapter.kt": "Button",
        "ImageAdapter.kt": "Image",
        "ToggleAdapter.kt": "Toggle",
        "CheckboxAdapter.kt": "Checkbox",
        "SliderAdapter.kt": "Slider",
        "SwitchAdapter.kt": "Switch",
        "ColumnAdapter.kt": "Column",
        "RowAdapter.kt": "Row",
        "ScrollViewAdapter.kt": "ScrollView",
        "WebViewAdapter.kt": "WebHost",
        "DatePickerAdapter.kt": "DatePicker",
        "PickerAdapter.kt": "Picker",
    }
    for path in sorted(ui_dir.rglob("*.kt")):
        if path.name == "PropsIndex.kt":
            continue
        text = path.read_text()
        comp = file_to_comp.get(path.name, "Unknown")
        # Match static: props.<accessor>(PropsIndex.CONSTANT)
        for m in re.finditer(r"props\.\w+\(PropsIndex\.(\w+)", text):
            const_name = m.group(1)
            if const_name in const_to_prop:
                prop_name = const_to_prop[const_name]
                comp_from_const = kotlin_const_to_comp(const_name)
                reads[comp_from_const].add(prop_name)
        # Match dynamic: props.<accessor>(PropsIndex.propIndexForName("propName"))
        for m in re.finditer(r'props\.\w+\(PropsIndex\.propIndexForName\("(\w+)"\)', text):
            prop_name = m.group(1)
            reads[comp].add(prop_name)
    return reads


def parse_swift_reads(swift_dir: pathlib.Path) -> dict[str, set[str]]:
    """Scan Swift adapter files for prop reads via props.value(for: .X)."""
    reads: dict[str, set[str]] = defaultdict(set)
    adapter_dir = swift_dir / "Sources/FluxUIKit"
    if not adapter_dir.exists():
        return reads
    file_to_comp = {
        "TextAdapter.swift": "Text",
        "TextInputAdapter.swift": "TextInput",
        "TextAreaAdapter.swift": "TextArea",
        "ButtonAdapter.swift": "Button",
        "ImageAdapter.swift": "Image",
        "ToggleAdapter.swift": "Toggle",
        "CheckboxAdapter.swift": "Checkbox",
        "SliderAdapter.swift": "Slider",
        "RowAdapter.swift": "Row",
        "ColumnAdapter.swift": "Column",
        "ScrollViewAdapter.swift": "ScrollView",
        "SwitchAdapter.swift": "Switch",
        "WebHostView.swift": "WebHost",
        "DatePickerAdapter.swift": "DatePicker",
        "PickerAdapter.swift": "Picker",
        "ScreenAdapter.swift": "Screen",
        "OverlayMotionAdapters.swift": "Dialog",
    }
    for path in sorted(adapter_dir.rglob("*.swift")):
        if path.name not in file_to_comp:
            continue
        comp = file_to_comp[path.name]
        text = path.read_text()
        # Match props.value(for: .propName)
        for m in re.finditer(r"props\.value\(for:\s*\.(\w+)\)", text):
            reads[comp].add(m.group(1))
        # Match new.getBool(named: "propName"), new.getString(named: "propName"), etc.
        for m in re.finditer(r"\.get\w+\(named:\s*\"(\w+)\"\)", text):
            reads[comp].add(m.group(1))
    return reads


def main() -> int:
    stdlib_dir = REPO_ROOT / "stdlib"
    kotlin_dir = REPO_ROOT / "adapters" / "ui-kotlin"
    swift_dir = REPO_ROOT / "adapters" / "ui-swift"

    declared = parse_stdlib_props(stdlib_dir)
    kotlin_reads = parse_kotlin_reads(kotlin_dir)
    swift_reads = parse_swift_reads(swift_dir)

    violations: list[str] = []
    all_components = (
        set(declared.keys()) | set(kotlin_reads.keys()) | set(swift_reads.keys())
    )

    for comp in sorted(all_components):
        comp_props = declared.get(comp, set())
        kt_reads = kotlin_reads.get(comp, set())
        sw_reads = swift_reads.get(comp, set())

        for prop in sorted(comp_props):
            in_kt = prop in kt_reads
            in_sw = prop in sw_reads
            if not in_kt and not in_sw:
                violations.append(
                    f"  {comp}.{prop}: declared but read by NEITHER kit"
                )
            elif in_kt ^ in_sw:
                which = "kotlin" if in_kt else "swift"
                violations.append(
                    f"  {comp}.{prop}: declared, read by only {which}"
                )

        # Props read but not declared
        for prop in sorted(kt_reads | sw_reads):
            if prop not in comp_props:
                violations.append(
                    f"  {comp}.{prop}: read by a kit but not declared"
                )

    if violations:
        print("VIOLATIONS:")
        for v in violations:
            print(v)
        return 1
    else:
        print("OK: stdlib↔kit prop contract is consistent.")
        return 0


if __name__ == "__main__":
    sys.exit(main())
