---
id: FLUX-048
status: todo
lane: LANE-Q
phase: "Phase 6"
blocked_by: []
labels:
  - capability
  - escape-hatch
source: CHANGELOG.md §roadmap §4 (WebView escape hatch capability, explicitly scoped)
related_adrs:
  - ADR-0045
---

# FLUX-048: WebView escape-hatch capability

- **Lane:** LANE-Q (Phase 6)
- **Depends on:** PRD-Q (capability contract)
- **Source:** `CHANGELOG.md` roadmap §4
- **Related ADRs:** ADR-0045

## Problem Statement

Roadmap §4: "WebView escape hatch capability, explicitly scoped as the 'when Flux
doesn't cover it' release valve — every framework needs one." Absent.

## Solution

A `WebView(src)` capability mapping to `WKWebView` (iOS) / `WebView` (Android) via
the ADR-0047 adapter contract, carrying a `src` (URL or html) prop + a message-
bridge callback prop for host↔web communication.

## Implementation Decisions

- Native webview, not a Flux-rendered screen; the bridge is a callback prop.
- Reuses the capability contract (denied grant → typed error).

## Testing Decisions

- Parity trace test: `WebView` maps to the expected native view on both backends; a
  JVM test asserts the bridge callback fires.

## Out of Scope

- The native-module escape hatch (FLUX-046) — that is for arbitrary SDKs, this is
  specifically web content.
