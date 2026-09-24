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

PROP_RE = re.compile(r"^\s*(\w+)\s*:\s*.*$", re.MULTILINE)


def parse_stdlib(stdlib_dir: Path) -> dict[str, set[str]]:
    """Return ``{component_name: {prop_name, ...}}`` from stdlib .flux files.

    Parses brace-delimited component declarations line-by-line: after a
    ``compo Name(`` line, prop lines are collected until a line whose stripped
    text is exactly ``)`` (the closing paren).  This correctly handles
    default-value expressions like ``fn() {}`` that contain parentheses.
    """
    props: dict[str, set[str]] = {}
    for path in sorted(stdlib_dir.glob("*.flux")):
        lines = path.read_text().splitlines()
        i = 0
        while i < len(lines):
            line = lines[i]
            m = re.match(r"\s*compo\s+(\w+)\s*\((.*)$", line)
            if not m:
                i += 1
                continue
            comp = m.group(1)
            prop_names: set[str] = set()
            rest = m.group(2).strip()
            # If the opening line already closes the paren (e.g.
            # `compo Foo(x: Int)`), parse props from the remainder.
            if rest.endswith(")"):
                prop_part = rest[:-1]
                pm = PROP_RE.match(prop_part)
                if pm:
                    prop_names.add(pm.group(1))
                props[comp] = prop_names
                i += 1
                continue
            if rest:
                pm = PROP_RE.match(rest)
                if pm:
                    prop_names.add(pm.group(1))
            # Read subsequent lines until we see a line that is exactly ")".
            i += 1
            while i < len(lines):
                stripped = lines[i].strip()
                if stripped == ")":
                    i += 1
                    break
                pm = PROP_RE.match(lines[i])
                if pm:
                    prop_names.add(pm.group(1))
                i += 1
            props[comp] = prop_names
    return props


# Components whose props are read by the native reconciler via FNV-1a
# hashing rather than by name in an adapter file.  The script cannot
# detect these reads statically; they are known-correct by design.
FNV_READ_PROPS: set[tuple[str, str]] = {
    ("Screen", "route"),   # FLUX-071: reconciler swaps the screen by route hash
    ("Text", "size"),      # Swift reads size via Font record field 1 (§3.2)
}

# Props declared in stdlib but not yet implemented in either kit's adapter.
# These are tracked gaps (FLUX-071: Stack.alignment reserved for ADR-0048).
KNOWN_UNREAD: set[tuple[str, str]] = {
    ("Stack", "alignment"),
}


# ---------------------------------------------------------------------------
# 2.  Kotlin kit — constant-prefix + shared-constant attribution
# ---------------------------------------------------------------------------

KOTLIN_PROPSINDEX = (
    REPO / "adapters" / "ui-kotlin"
    / "src/main/kotlin/dev/flux/ui/PropsIndex.kt"
)

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
    ("IMAGE", "Image"),
    ("MODAL", "Modal"),
    ("PANEL", "Panel"),
    ("PICKER", "Picker"),
    ("ROW", "Row"),
    ("ROUTER", "Router"),
    ("SAFEAREA", "SafeArea"),
    ("SCREEN", "Screen"),
    ("SCROLL", "ScrollView"),
    ("SCROLLVIEW", "ScrollView"),
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
    "STACK_ALIGNMENT": {"Column", "Row"},
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
            prop = const_to_prop.get(const, const)
            if const in SHARED_CONSTANTS:
                for comp in SHARED_CONSTANTS[const]:
                    reads.setdefault(comp, set()).add(prop)
                continue
            comp = kotlin_const_to_comp(const)
            if comp is None or comp == "":
                continue
            reads.setdefault(comp, set()).add(prop)

    # Scan adapter files for dynamic  props.<accessor>(PropsIndex.propIndexForName("prop"))
    for path in sorted(ui_dir.glob("*.kt")):
        text = path.read_text()
        file_comp = kotlin_file_to_comp.get(path.name)
        if not file_comp:
            continue
        for m in re.finditer(
            r'props\.\w+\(PropsIndex\.propIndexForName\("(\w+)"\)\)', text
        ):
            reads.setdefault(file_comp, set()).add(m.group(1))

    return reads


# Kotlin adapter files whose component can't be derived from a PropsIndex
# constant prefix (e.g. dynamic propIndexForName reads).
kotlin_file_to_comp: dict[str, str] = {
    "WebViewAdapter.kt": "WebHost",
    "FluxUiKit.kt": "",          # registration table — skip
    "PropsIndex.kt": "",          # definition file — skip (handled above)
}


# ---------------------------------------------------------------------------
# 3.  Swift kit — file-based attribution
# ---------------------------------------------------------------------------

SWIFT_PROP_RE = re.compile(
    r'\w+\.get\w+\(named:\s*"(\w+)"\)'
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
    "ScrollViewAdapter.swift": ["ScrollView"],
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
    # Multi-component files — reads here are attributed to ALL listed comps,
    # but the props they carry are treated as unreliable for "not declared"
    # checks (see *multi_comps* tracking below).
    "LayoutAdapters.swift": ["Stack", "Spacer", "SafeArea", "Grid"],
    "OverlayMotionAdapters.swift": ["Dialog", "Modal", "Sheet", "Animate"],
}


def scan_swift(swift_dir: Path) -> tuple[dict[str, set[str]], set[str]]:
    """Return ``({component: {prop, ...}}, multi_comp_unreliable)``.

    *reads* maps each component to the set of prop names it reads on the Swift
    side.  *multi_comps* lists components backed by multi-component Swift files
    — for those, prop-name reads are **unreliable** (over-attributed) and must
    not be used to flag "read by a kit but not declared" on the Swift side.
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


def is_handler(prop: str) -> bool:
    """A prop whose name starts with ``on`` is treated as a Handler callback."""
    return prop.startswith("on")


def main() -> int:
    stdlib = parse_stdlib(REPO / "stdlib")
    kt_reads = scan_kotlin(REPO / "runtimes" / "android")
    sw_reads, sw_multi = scan_swift(REPO / "adapters" / "ui-swift")

    violations: list[str] = []

    all_comps: set[str] = set(stdlib) | set(kt_reads) | set(sw_reads)

    for comp in sorted(all_comps):
        declared = stdlib.get(comp, set())
        in_kt = kt_reads.get(comp, set())
        in_sw = sw_reads.get(comp, set())

        # Skip non-component utility entries that appear in neither stdlib nor
        # the kits' adapter files.
        if not declared and not in_kt and not in_sw:
            continue

        # (1) declared but read by NEITHER kit
        for prop in sorted(declared):
            if (comp, prop) in FNV_READ_PROPS or (comp, prop) in KNOWN_UNREAD:
                continue
            is_h = is_handler(prop)
            kt_has = prop in in_kt or is_h
            sw_has = prop in in_sw or (is_h and "__handlers__" in in_sw)
            if not kt_has and not sw_has:
                violations.append(
                    f"{comp}.{prop}: declared but read by NEITHER kit"
                )

        # (2) read by a kit but not declared
        #     Only flag Kotlin constant-prefix reads and Swift reads from
        #     SINGLE-component files (reliable).  Swift multi-component file
        #     reads are unreliable and skipped here.
        sw_reliable = comp not in sw_multi
        for prop in sorted(in_kt):
            if prop == "__handlers__":
                continue
            if prop not in declared:
                violations.append(
                    f"{comp}.{prop}: read by Kotlin but not declared"
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
            if (comp, prop) in FNV_READ_PROPS or (comp, prop) in KNOWN_UNREAD:
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


if __name__ == "__main__":
    sys.exit(main())
