# Reconcile trace format v1

**Path:** docs/spec/reconcile-trace-format.md · **Owners:** parity agent (tooling + goldens), P1/P2 (emitters)

See ADR-0027 for the invariant this format proves observably and cross-host.

## Purpose

Prove ADR-0027's invariant observably and cross-host: the same frame + dispatch script run against Swift and Kotlin hosts must produce byte-identical (post-normalization) traces.

## Event grammar (JSONL, one event per line, canonical JSON: sorted keys, no whitespace)

```text
event        := frame | apply_patch | dispatch | signals | dirty | build | update
              | skip_unchanged | skip_pure | skip_pruned | detach | setchildren
              | reorder | mount | cleanup | error | step_end

frame        := {"t":"frame","seq":N,"full":bool,"root":id?,"nodes":N,"patches":N}
apply_patch  := {"t":"apply_patch","seq":N,"patches":N}
dispatch     := {"t":"dispatch","handler":id}
signals      := {"t":"signals","ids":[id,...]}       // VM-written, ascending
dirty        := {"t":"dirty","ids":[id,...]}         // post-prune, visit order (depth,id)
build        := {"t":"build","id":id}
update       := {"t":"update","id":id}
skip_unchanged := {"t":"skip_unchanged","id":id}
skip_pure    := {"t":"skip_pure","id":id}
skip_pruned  := {"t":"skip_pruned","id":id}          // Phase 2 subtree early-out
detach       := {"t":"detach","id":id}
setchildren  := {"t":"setchildren","id":id,"n":N}
reorder      := {"t":"reorder","id":id,"keys":[k,...]}
mount        := {"t":"mount","id":id}
cleanup      := {"t":"cleanup","id":id}
error        := {"t":"error","kind":str,"offset":N}
step_end     := {"t":"step_end","i":N,"built":N,"updated":N,
                 "skipped_unchanged":N,"skipped_pure":N,"detached":N,
                 "prop_materializations":N}
```

Every script step emits a terminating `step_end` with cumulative counters. `prop_materializations` is the R2 smoking gun: it must go `3N → ≤ 2·changed + built`.

## Dispatch script format

```json
{
  "name": "counter_1000",
  "phase": 3,
  "frames": [{"file": "fixtures/counter_1000.init.fluxbin"}],
  "steps": [
    {"op": "apply",        "frame": 0},
    {"op": "dispatch",     "handler": 7, "payload": {"int": 1}},
    {"op": "apply_patch",  "frame": 1},
    {"op": "snapshot"}
  ]
}
```

**Payload restriction (normative):** dispatch payloads are limited to `int` / `bool` / `string` / `null`. No floats — this dodges f64 formatting divergence between Swift and Kotlin JSON encoders. Enforce in the script loader.

## Host hook points

- **Swift:** `ShadowTreeReconciler` gains `var trace: ((TraceEvent) -> Void)?` (nil in prod, INV-2). A test-target `TraceDriver` loads the script, drives `FluxRuntime.apply` / `dispatch`, dumps `trace.swift.jsonl`.
- **Kotlin:** `ShadowTree` gains `var trace: ((TraceEvent) -> Unit)?`; `TraceDriver` in the host test target drives `FluxExecutor` via injected test dispatcher (per ADR-0027 threading).
- Frame fixtures: handcrafted minimal frames until the server agent ships a fixture generator (flag as its own task); generation thereafter from dev-server snapshots.

## Comparison tool

`flux-parity trace diff --phase {1,2,3} trace.swift.jsonl trace.kotlin.jsonl`

- Canonicalizes lines, compares exactly, exits non-zero on first divergence with line context.
- `--phase` selects the golden set (Phase 1 traces lack `dirty`/`skip_pruned`; Phase 2 adds them; Phase 3 adds thunk-driven events). Goldens live in `/tests/trace-goldens/<scenario>.<phase>.jsonl`.

## Golden scenarios

| Scenario | Setup | Assertion |
|---|---|---|
| `counter_1000` | 1,000-node static tree, one `Text` reads signal 1 | After dispatch: `dirty` = exactly the Text's id; 1 update, 0 builds |
| `noop_dispatch` | Handler writes signal 99, nothing reads it | `dirty: []`, all step_end deltas zero |
| `pure_subtree` | `@pure` subtree; sibling signal written | Zero events inside the pure subtree |
| `cond_flip` | `If` condition signal flips | build+detach = branch children counts; exactly 1 `setchildren` on the If's parent; mount/cleanup fire once each |
| `foreach_grow` | Collection signal grows | Builds only for new splice items (**OQ-3 gated**) |
| `unrelated_signal` | Signal written, in no node's deps | Zero update/build/detach |

**Example golden — `counter_1000`, Phase 3, dispatch step:**

```json
{"t":"dispatch","handler":7}
{"t":"signals","ids":[1]}
{"t":"dirty","ids":[57]}
{"t":"update","id":57}
{"t":"step_end","i":1,"built":0,"updated":1,"skipped_unchanged":0,"skipped_pure":0,"detached":0,"prop_materializations":2}
```

**Example golden — `noop_dispatch`:**

```json
{"t":"dispatch","handler":8}
{"t":"signals","ids":[99]}
{"t":"dirty","ids":[]}
{"t":"step_end","i":2,"built":0,"updated":0,"skipped_unchanged":0,"skipped_pure":0,"detached":0,"prop_materializations":0}
```
