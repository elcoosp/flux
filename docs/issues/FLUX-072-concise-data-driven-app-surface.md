---
id: FLUX-072
status: partial
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

# FLUX-072: Concise data-driven app surface

> **Status note (2026-08-29):** relabeled `todo` → `partial`. An uncommitted WIP
> (18 files) is actively implementing this: list VM ops (`ListInsert`/`ListRemove`/
> `ListClear`) merged into `flux-ir`/`flux-vm-ref`; `ForEach` now lowers a real
> `Child::Splice` carrying `(key, child_id)` pairs (was empty); `$`-binding wired
> through the type checker. Not yet merged to `main`. Backlog items `$` two-way
> binding, parameterized `compo` typed props, and `Toggle`/`Spacer weight:` remain. (lists, records, two-way bindings, parameterized components)

- **Lane:** LANE-L (language maturity)
- **Depends on:** FLUX-051 (ForEach/iteration), FLUX-052 (slot/children), FLUX-054 (prop typing), FLUX-040 (form primitives)
- **Source:** the `examples/todo` example. It currently needs 111 lines of five fixed signal slots (`t0..t4` / `d0..d4`), a 5-deep `when added == N` ladder, five duplicated TaskRow blocks, and a manual `newTask` relay signal — all because the language lacks collections, records, two-way input binding, and parameterized components.
- **Related ADRs:** ADR-0050 (ForEach as reactive collection — see verification note below), ADR-0047 (codegen registry).

## Problem Statement

The MLP primitives render, but the *language* cannot express a real app concisely. The To-Do example — meant to showcase the framework — is a slot-ladder workaround. The gaps are language/runtime, not example authoring, failures.

### Verification note (re-grounded 2026-08-30)

The earlier claim that `ForEach` emits an empty `Child::Splice` and that the VM
has no list ops is **stale** — the code has since landed:

- `crates/flux-ir/src/lower/mod.rs:486` now lowers `ForEach` to a **real**
  `Child::Splice` carrying `(key, child_id)` pairs (was `items: vec![]`).
- `crates/flux-vm-ref/src/vm.rs` has `ListPush` / `ListInsert` / `ListRemove` /
  `ListClear` / `ListRemoveItem` ops; `crates/flux-ir/src/lower/bytecode.rs`
  emits `LIST_INSERT` etc.
- The `$name` two-way binding sigil is resolved in the type checker
  (`flux-types/src/checker.rs:897`) **and** emitted as a write-back in the
  lowering pass (`flux-ir/src/lower/bytecode.rs:1104`). `examples/todo` uses
  `$newTask` and compiles through the real `Pipeline`.

So the data-driven core (records, `ForEach` by identity, `derived`, `$` two-way
binding, `List` mutation) is **done end-to-end**. What genuinely remains before
this issue can close:

- **Parameterized `compo` typed props** — generic component instantiation exists
  in `flux-ir/src/lower/mono.rs` but the `.flux` authoring surface + the
  `examples/todo` rewrite to express `TaskRow(task: Task)` as a parameterized
  component (instead of the current workaround) is unfinished.
- The todo example still uses the pre-`FLUX-072` shape for some parts; rewrite to
  the concise form once parameterized compo props land.

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
