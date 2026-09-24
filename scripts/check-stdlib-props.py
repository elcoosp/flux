#!/usr/bin/env python3
"""
Stdlib↔kit prop contract auditor (T-503).

Scans ``stdlib/*.flux`` for component prop declarations, scans the Kotlin and
Swift adapter kits for which props each reads (not merely declares), and reports
three classes of drift:

  * ``declared but read by NEITHER kit``  — dead prop, remove from stdlib.
  * ``declared and read by only ONE kit``  — cross-platform gap.
  * ``read by a kit but not declared``     — kit out of sync with stdlib.

Exit 0 only when no violations remain.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# ---------------------------------------------------------------------------
# 1.  Parse stdlib/*.flux  →  {component: set(prop_name)}
# ---------------------------------------------------------------------------

PROP_RE = re.compile(r"^\s*(\w+)\s*:\s*(\w+)", re.MULTILINE)
# Match  `compo Name(prop: Type, ...)`  including the multi-line brace form.
COMPO_RE = re.compile(
    r"^\s*compo\s+(\w+)\s*\((.*?)\)\s*(\{|$)",
    re.MULTILINE | re.DOTALL,
)

# Components whose props are read by the native reconciler via FNV-1a
# hashing rather than by name in an adapter file.  The script cannot
# detect these reads statically; they are known-correct by design.
FNV_READ_PROPS: set[tuple[str, str]] = {
    ("Screen", "route"),   # FLUX-071: reconciler swaps the screen by route hash
}


def parse_stdlib(stdlib_dir: Path) -> dict[str, set[str]]:
    """Return ``{component_name: {prop_name, ...}}`` from stdlib .flux files."""
    props: dict[str, set[str]] = {}
    for path in sorted(stdlib_dir.glob("*.flux")):
        text = path.read_text()
        for m in COMPO_RE.finditer(text):
            comp = m.group(1)
            body = m.group(2)
            prop_names: set[str] = set()
            for pm in PROP_RE.finditer(body):
                prop_names.add(pm.group(1))
            props[comp] = prop_names
    return props


# ---------------------------------------------------------------------------
# 2.  Kotlin kit — constant-prefix + shared-constant attribution
# ---------------------------------------------------------------------------

KOTLIN_PROPSINDEX = (
    REPO / "adapters" / "ui-kotlin"
    / "src/main/kotlin/dev/flux/ui/PropsIndex.kt"
)

# PropsIndex constant-name → prop-name  (built from propIndexForName("name"))
# Also: constant-name → prop-name for FNV-read props (handled separately).

# Prefix → component for PropsIndex constant attribution.
# Ordered so longer prefixes match first.
KOTLIN_PREFIXES = [
    ("ANIMATE", "Animate"),
    ("A11Y", ""),          # skip — read via host-level FLUX-044 helpers
    ("BUTTON", "Button"),
    ("CHECKBOX", "Checkbox"),
    ("COLUMN", "Column"),
    ("DATE_PICKER", "DatePicker"),
    ("DIALOG", "Dialog"),
    ("GESTURE", "Gesture"),
    ("GRID", "Grid"),
    ("MODAL", "Modal"),
    ("ON_DISMISS", "Dialog"),         # fallback for OVERLAY_ON_DISMISS
    ("PANEL", "Panel"),
    ("PICKER", "Picker"),
    ("ROW", "Row"),
    ("ROUTER", "Router"),
    ("SAFEAREA", "SafeArea"),
    ("SCREEN", "Screen"),
    ("SHEET", "Sheet"),
    ("SLIDER", "Slider"),
    ("SPACER", "Spacer"),
    ("STACK", "Stack"),
    ("SWITCH", "Switch"),
    ("TEXT_AREA", "TextArea"),
    ("TEXT_INPUT", "TextInput"),
    ("TEXT", "Text"),
    ("TOGGLE", "Toggle"),
    ("WEBHOST", "WebHost"),
    ("WEBVIEW", "WebView"),
]


def kotlin_const_to_comp(const_name: str) -> str | None:
    """Map a PropsIndex constant name to its owning component, or None to skip."""
    for prefix, comp in KOTLIN_PREFIXES:
        if const_name.startswith(prefix + "_"):
            return comp
    return None


# Constants whose prop is genuinely shared across several components because
# the same layout constant is reused by multiple adapters.
# Format: constant_name → set(component, ...)
SHARED_CONSTANTS: dict[str, set[str]] = {
    "STACK_GAP": {"Column", "Row", "Stack", "Grid"},
    "STACK_ALIGNMENT": {"Column", "Row", "Stack", "Grid"},
    "OVERLAY_ON_DISMISS": {"Dialog", "Modal", "Sheet"},
}


def scan_kotlin(kotlin_dir: Path) -> dict[str, set[str]]:
    """Return ``{component: {prop_name, ...}}`` of props each Kotlin kit reads."""
    idx_text = KOTLIN_PROPSINDEX.read_text()

    # Build constant-name → prop-name from propIndexForName("propName")
    idx_pattern = re.compile(
        r"val\s+(\w+)\s*:\s*UShort\s*=\s*propIndexForName\(\"(\w+)\"\)"
    )
    const_to_prop: dict[str, str] = {}
    for m in idx_pattern.finditer(idx_text):
        const_to_prop[m.group(1)] = m.group(2)

    reads: dict[str, set[str]] = {}

    # Scan adapter files for static  props.<accessor>(PropsIndex.CONSTANT)
    ui_dir = REPO / "adapters" / "ui-kotlin" / "src/main/kotlin/dev/flux/ui"
    for path in sorted(ui_dir.glob("*.kt")):
        text = path.read_text()
        for m in re.finditer(r"props\.\w+\(PropsIndex\.(\w+)", text):
            const = m.group(1)
            if const in SHARED_CONSTANTS:
                for comp in SHARED_CONSTANTS[const]:
                    reads.setdefault(comp, set()).add(
                        const_to_prop.get(const, const)
                    )
                continue
            comp = kotlin_const_to_comp(const)
            if comp is None or comp == "":
                continue
            reads.setdefault(comp, set()).add(
                const_to_prop.get(const, const)
            )

    # Scan adapter files for dynamic  props.<accessor>(PropsIndex.propIndexForName("prop"))
    for path in sorted(ui_dir.glob("*.kt")):
        text = path.read_text()
        file_comp = kotlin_file_to_comp.get(path.name)
        if file_comp is None:
            continue
        for m in re.finditer(
            r'props\.\w+\(PropsIndex\.propIndexForName\("(\w+)"\)\)', text
        ):
            reads.setdefault(file_comp, set()).add(m.group(1))

    return reads


# Kotlin adapter files whose component can't be derived from a PropsIndex
# constant prefix (e.g. dynamic propIndexForName reads, or multi-component files).
kotlin_file_to_comp: dict[str, str] = {
    "WebViewAdapter.kt": "WebHost",
    "FluxUiKit.kt": "",          # registration table — skip
    "PropsIndex.kt": "",          # definition file — skip (handled above)
}


# ---------------------------------------------------------------------------
# 3.  Swift kit — file-based attribution
# ---------------------------------------------------------------------------

SWIFT_HANDLED_RE = re.compile(r"public func bindHandler", re.MULTILINE)
SWIFT_PROP_RE = re.compile(
    r'\w+\.get\w+\(named:\s*"(\w+)"\)'   # new.getFloat(named: "gap") or props.get(...)
)
SWIFT_VALUE_RE = re.compile(r'\.value\(for:\s*\.(\w+)\)')
SWIFT_HANDLER_RE = re.compile(r"bindHandler\b")


# Swift adapter file → list of components it implements.
# Files serving a SINGLE component get a 1-element list.
swift_file_to_comps: dict[str, list[str]] = {
    "ButtonAdapter.swift": ["Button"],
    "CheckboxAdapter.swift": ["Checkbox"],
    "ColumnAdapter.swift": ["Column"],
    "DatePickerAdapter.swift": ["DatePicker"],
    "GestureAdapter.swift": ["Gesture"],
    "GridAdapter.swift": ["Grid"],
    "ImageAdapter.swift": ["Image"],
    "ModalAdapter.swift": ["Modal"],
    "PanelAdapter.swift": ["Panel"],
    "PickerAdapter.swift": ["Picker"],
    "RowAdapter.swift": ["Row"],
    "SafeAreaAdapter.swift": ["SafeArea"],
    "ScreenAdapter.swift": ["Screen"],
    "SheetAdapter.swift": ["Sheet"],
    "SliderAdapter.swift": ["Slider"],
    "SpacerAdapter.swift": ["Spacer"],
    "StackAdapter.swift": ["Stack"],
    "SwitchAdapter.swift": ["Switch"],
    "TextAdapter.swift": ["Text"],
    "TextAreaAdapter.swift": ["TextArea"],
    "TextInputAdapter.swift": ["TextInput"],
    "ToggleAdapter.swift": ["Toggle"],
    "WebHostView.swift": ["WebHost"],
    # Multi-component files — reads here are attributed to ALL listed comps.
    "LayoutAdapters.swift": ["Stack", "Spacer", "SafeArea", "Grid"],
    "OverlayMotionAdapters.swift": ["Dialog", "Modal", "Sheet", "Animate"],
}


def scan_swift(swift_dir: Path) -> tuple[dict[str, set[str]], set[str]]:
    """Return ``({component: {prop, ...}}, multi_comp_unreliable)``.

    *reads* maps each component to the set of prop names it reads on the Swift
    side.  *multi_comps* is the set of components that are backed by a
    multi-component Swift file — for those, ``prop`` reads are **unreliable**
    (over-attributed), so they must not be used to flag "read by a kit but
    not declared" on the Swift side.
    """
    swift_src = swift_dir / "Sources/FluxUIKit"
    reads: dict[str, set[str]] = {}
    multi_comps: set[str] = set()

    for path in sorted(swift_src.glob("*.swift")):
        text = path.read_text()
        comps = swift_file_to_comps.get(path.name)
        if comps is None:
            continue
        if len(comps) > 1:
            for c in comps:
                multi_comps.add(c)

        has_handler = bool(SWIFT_HANDLER_RE.search(text))

        for c in comps:
            comp_reads = reads.setdefault(c, set())
            for m in SWIFT_PROP_RE.finditer(text):
                comp_reads.add(m.group(1))
            for m in SWIFT_VALUE_RE.finditer(text):
                comp_reads.add(m.group(1))
            if has_handler:
                comp_reads.add("__handlers__")

    return reads, multi_comps


# ---------------------------------------------------------------------------
# 4.  Audit
# ---------------------------------------------------------------------------

VOWEL = "AEIOU"


def is_handler(prop: str) -> bool:
    return prop.startswith("on")


def main() -> int:
    stdlib = parse_stdlib(REPO / "stdlib")
    kt_reads = scan_kotlin(REPO / "runtimes" / "android")
    sw_reads, sw_multi = scan_swift(REPO / "adapters" / "ui-swift")

    violations: list[str] = []

    # Collect the universe of components that matter.
    all_comps: set[str] = set(stdlib) | set(kt_reads) | set(sw_reads)

    for comp in sorted(all_comps):
        declared = stdlib.get(comp, set())
        in_kt = kt_reads.get(comp, set())
        in_sw = sw_reads.get(comp, set())

        # Skip components that aren't declared in stdlib and aren't read
        # meaningfully (they are non-component utility code).
        if not declared and not in_kt and not in_sw:
            continue

        # (1) declared but read by NEITHER kit
        for prop in sorted(declared):
            if (prop, comp) in FNV_READ_PROPS:
                continue
            kt_has = prop in in_kt
            sw_has = prop in in_sw
            if not kt_has and not sw_has:
                violations.append(
                    f"{comp}.{prop}: declared but read by NEITHER kit"
                )

        # (2) read by a kit but not declared
        #     Only flag Swift reads from SINGLE-component files (reliable).
        #     Kotlin constant-prefix reads are always reliable.
        #     Handler reads are excluded (detected via bindHandler sentinel).
        sw_reliable = comp not in sw_multi
        for prop in sorted(in_kt):
            if prop == "__handlers__":
                continue
            if prop not in declared and prop not in all_comps_for_prop(prop):
                violations.append(
                    f"{comp}.{prop}: read by a kit but not declared"
                )
        if sw_reliable:
            for prop in sorted(in_sw):
                if prop == "__handlers__":
                    continue
                if prop not in declared:
                    violations.append(
                        f"{comp}.{prop}: read by Swift but not declared"
                    )

        # (3) declared and read by only ONE kit
        for prop in sorted(declared):
            if (prop, comp) in FNV_READ_PROPS:
                continue
            is_h = is_handler(prop)
            kt_has = prop in in_kt or is_h
            sw_has = prop in in_sw or (is_h and "__handlers__" in in_sw)
            if kt_has != sw_has:
                kit = "kotlin" if kt_has else "swift"
                violations.append(
                    f"{comp}.{prop}: declared, read by only {kit}"
                )

    if violations:
        print("VIOLATIONS:")
        for v in violations:
            print(f"  {v}")
        print(f"\n{len(violations)} violation(s)")
        return 1

    print("All component prop contracts satisfied.")
    return 0


def all_comps_for_prop(_prop: str) -> set[str]:
    """Placeholder — kept for future per-prop component allow-list."""
    return set()


if __name__ == "__main__":
    sys.exit(main())
