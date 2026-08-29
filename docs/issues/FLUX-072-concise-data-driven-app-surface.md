---
id: FLUX-072
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - FLUX-051
  - FLUX-052
  - FLUX-054
  - FLUX-040
labels:
  - language
  - types
  - dx
  - stdlib
source: examples/todo (111-line slot-ladder) + user review "what a ugly mess" — the example is forced to fake data modelling the framework does not yet support.
related_adrs:
  - ADR-0050
  - ADR-0047
---

# FLUX-072: Concise data-driven app surface (lists, records, two-way bindings, parameterized components)

- **Lane:** LANE-L (language maturity)
- **Depends on:** FLUX-051 (ForEach/iteration), FLUX-052 (slot/children), FLUX-054 (prop typing), FLUX-040 (form primitives)
- **Source:** the `examples/todo` example. It currently needs 111 lines of five fixed signal slots (`t0..t4` / `d0..d4`), a 5-deep `when added == N` ladder, five duplicated TaskRow blocks, and a manual `newTask` relay signal — all because the language lacks collections, records, two-way input binding, and parameterized components.
- **Related ADRs:** ADR-0050 (ForEach as reactive collection — see verification note below), ADR-0047 (codegen registry).

## Problem Statement

The MLP primitives render, but the *language* cannot express a real app concisely. The To-Do example — meant to showcase the framework — is a slot-ladder workaround. The gaps are language/runtime, not example authoring, failures.

### Verification note (must resolve before closing)
FLUX-051 is marked **done** via ADR-0050, but the actual lower pass still emits an **empty** `Child::Splice`:

- `crates/flux-ir/src/lower/mod.rs:468` → `children: vec![Child::Splice { items: vec![] }]`
- `crates/flux-vm-ref/src` has **no** list ops (`APPEND` / `LIST_GET` / `LIST_LEN` / `LIST_REMOVE` / `LIST_CLEAR`).

So ForEach is parsed and type-checked but does not yet carry data end-to-end. The closure text in FLUX-051 contradicts the code; this issue owns reconciling them.

## Target: the example rewritten concisely

```
compo TaskRow(task: Task)
    Row gap: 8
        Toggle checked: task.done, onToggle: || { task.done = !task.done }
        Text text: task.label
        Spacer weight: 1
        Button text: "Remove", onPress: || { tasks.remove(task) }

compo TodoApp
    state tasks: List<Task> = [
        Task(label: "Buy groceries"), Task(label: "Walk the dog"),
        Task(label: "Read a chapter"), Task(label: "Reply to emails"),
        Task(label: "Water the plants"),
    ]
    state newTask: String = ""

    Column gap: 16
        Text text: "Flux To-Do"
        Row gap: 8
            TextInput text: $newTask, placeholder: "What needs doing?"
            Button text: "Add task", onPress: || {
                tasks.append(Task(label: newTask))
                newTask = ""
            }
        Row gap: 8
            Button text: "Reset", onPress: || { tasks.clear() }
            Button text: "About", onPress: || { Router.navigate("about") }
        if tasks.isEmpty { Text text: "Nothing yet — add your first task" }
        ForEach tasks, TaskRow
```

~28 lines vs the current 111. Every removed line maps to a missing feature below.

## Missing features (the implementation backlog)

1. **`List<T>` value type + construction** — `TcType::List` exists in `crates/flux-types/src/kind.rs`, but there is no list *literal* lowering and no list *value* representation the VM can hold/mutate.
2. **VM list operations** — add `MAKE_LIST`, `LIST_APPEND`, `LIST_GET`, `LIST_LEN`, `LIST_REMOVE` (by index and by keyed item), `LIST_CLEAR`, `LIST_INSERT`. Wire/protocol + both host VMs (`flux-vm-ref` oracle first, then Swift/Kotlin).
3. **Records / structs** — `Task(label: String, done: Bool)` with named-field literals, field read, and field mutation (`task.done = !task.done`). Pairs with FLUX-054 (prop typing) for component-prop reuse.
4. **`$binding` two-way input sugar** — `TextInput text: $newTask` desugars to a framework-owned signal the adapter writes back through the VM on each keystroke (immediate local echo, source of truth in the signal graph). Removes the manual `onChangeText: |t| { newTask = t }` relay and the per-host local-state buffering hack (Android `ShadowTreeRenderer.RenderTextInput` currently needs a `MutableState` because a naive controlled `TextField` snaps back to the stale prop).
5. **Real `ForEach` lowering** — populate `Child::Splice` with the actual item nodes and emit per-item handler closures that *capture the item* (not just a payload register). Hosts already do keyed reconciliation by `nodeId`, so the missing piece is the IR. Reconcile FLUX-051's "done" claim against the empty-splice reality.
6. **Parameterized `compo` with typed props** — `compo TaskRow(task: Task)`; prop typing per FLUX-054. Removes the five duplicated row blocks.
7. **`Toggle` primitive** — part of FLUX-040 (form primitives, currently `partial`).
8. **`Spacer weight:` layout helper** — trivial; needed so `Toggle`/`Text`/`Remove` lay out like a real row.
9. **Struct field mutation + `!` / negation operator** — `task.done = !task.done`.
10. **Keyed list identity for splicing** — `Child::Splice` must carry a stable item key (derive from a `key:` field or item identity) so a `ForEach` reconciles by identity, preserving per-row `Toggle`/focus across reorder/remove.
11. **`if <cond> { … }` as an inline expression** — conditional rendering without a separate component (`if tasks.isEmpty { … }`).
12. **`derived` computed signals** — `derived` is already a reserved keyword (`crates/flux-parser/src/lexer.rs:79`) but unwired. Allows `remaining = tasks.filter(notDone).count` instead of a manually-bumped `added` counter that can desync.

## Load-bearing three (make the example *possible* at all)

- **#1 + #2 lists & VM ops** — no data model without them.
- **#4 `$` two-way binding** — makes `TextInput` ergonomic and removes the per-host snapping-back hack.
- **#5 real ForEach** — renders the list.

## Acceptance

`examples/todo` rewrites to the ~28-line form above (or equivalent), builds on both hosts, and the running app: types into the input (value sticks, no local-state hack), "Add task" appends a row and clears the field, "Remove"/"Toggle" mutate the correct item by identity, "Reset" clears the list, and Router nav still works. Both host VMs pass new `flux-parity` goldens for list append/remove/render and two-way input binding.

## Non-goals

- Async/await capability ergonomics beyond what FLUX-055 (Result/error) already covers.
- Router route params / typed navigation (separate concern).
- Adapter-kit *completeness* bugs (iOS ImageAdapter, Android TextInput renderer) — tracked elsewhere; this issue is language/runtime only.
