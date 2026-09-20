# Fix-Playbook Progress

## Baseline (R6)
- cargo build: ok
- cargo test: 1 pre-existing failure (b37_pure_parity — unrelated to playbook)
- gradlew :host:testDebugUnitTest: ok
- xcodebuild FluxHost: build fails — UIKit can't resolve on macOS (iOS-only framework)

## Phase 0 — COMMITTED
| Task | Commit | Description |
|---|---|---|
| T-001 | (pre-existing) | Contract-freeze manifest already existed |
| T-002 | (pre-existing) | Gradle wrapper already present |
| T-003 | `423b5d4` | Removed phantom test-module declarations |
| T-004 | `e104a7d` | Repaired force-unwrap regex bracket-class bug |
| T-005 | `e083d53` | Dropped stale manifest row + steward TOML validation |
| T-006 | `566af6a` | Un-ignored fuzz corpus |
| T-007 | `566af6a` | Removed dead generate_for_each_hashes.sh |
| T-008 | `2f8766c` | Added permissions+concurrency to all 16 workflows |

## Phase 1 — IN PROGRESS
| Task | Status | Commit | Description |
|---|---|---|---|
| T-101 | DONE | `0b424f1` | Register allocator fails loudly on exhaustion |
| T-102 | DONE | `0b424f1` | Statement-scope watermark reuses scratch registers |
| T-103 | **BLOCKED** | — | Typed opcode selection requires type map at lowering input (pipeline redesign) |
| T-104 | DONE | `26a38ec` | `?.` uses prop_index_for_name |
| T-105 | DONE | `1632779` | Literal match arms compare values |
| T-106 | DONE | `b6f10b2` | Variant tag in record field 0 on both paths |
| T-107 | **BLOCKED** | — | Component inline NodeId duplication — HIGH danger, needs careful design of call-site salting through lower_block → lower_expr → expr_node_id chain. Audit says "do not improvise beyond the steps." |
| T-108 | PENDING | — | Lexer negative literals |
| T-109 | PENDING | — | Generics shared supply |
| T-110 | PENDING | — | Discarded type errors |

## Sidecars
- T-103 BLOCKED: no type map at lowering input
- T-107 BLOCKED: architectural, needs call-site id threaded through entire inline path
