# FLUX-F66 — `referenced_strings_and_handlers` drops `Patch::Reattach`'s node

**Status:** OPEN (fix landed in same commit)
**Found:** round-112 bug hunt (`flux-devserver/src/pipeline.rs`)
**Severity:** low (latent: the only current caller is the dispatch-report
path, which only emits `Patch::Update`. A future caller that feeds a
Reattach-bearing delta through the walker — e.g. the hot-reload path if
it is refactored to share the walker — would ship a delta missing the
strings and handlers the reattached node references.)

## Summary

`crates/flux-devserver/src/pipeline.rs::referenced_strings_and_handlers`
walks a `Patch` stream and collects the strings and handlers each patch
refers to:

    for p in patches {
        match p {
            Patch::Replace { node, .. } | Patch::Insert { node, .. } => {
                walk_node(node, &mut strings, &mut handlers);
            }
            Patch::Update { props_diff, .. } => { … }
            Patch::Handler { id, .. } => { handlers.insert(*id); }
            Patch::Remove { .. } | Patch::Reorder { .. } => {}
            _ => {}                         // <- Reattach lands here
        }
    }

`Patch::Reattach { old_id, new_id, node }` carries a `NodeRef` — the
same shape as `Replace`/`Insert` — whose `props` may hold string
references and whose `handlers` list is a set of `HandlerId`s. The
walker's `_ => {}` wildcard silently drops all of that.

The walker's own doc says:

> The walk is total over the public `Patch` and `Value` variants; a
> wildcard arm future-proofs it against new variants that carry no
> strings/handlers.

That claim is wrong today: `Patch::Reattach` is a public variant, it
carries strings/handlers, and the wildcard drops them.

## Why it is latent today

The only caller of `referenced_strings_and_handlers` is
`try_build_dispatch_delta` (see the function's doc). The dispatch path
emits only `Patch::Update` — `emit_minimal_updates` returns a
`Vec<Patch::Update>` and nothing else. So `Reattach` never reaches the
walker today. The bug is dormant.

## Impact if a caller changes

`try_build_delta` (the hot-reload delta builder) ships *all* strings
and closures, not a scoped subset, so it does not use this walker.
If a future refactor makes it share the walker (to shrink the hot-reload
frame the same way it shrank the dispatch frame), any `Reattach`
patch's referenced strings/handlers would be missing from the delta.
The host would then see references to string ids or handler ids that
were not shipped with the frame and either display wrong text or fail
to dispatch a handler.

## Fix

Add `Patch::Reattach { node, .. }` to the walker's node-carrying arm:

    Patch::Replace { node, .. }
    | Patch::Insert { node, .. }
    | Patch::Reattach { node, .. } => {
        walk_node(node, &mut strings, &mut handlers);
    }

## Regression tests

`crates/flux-devserver/tests/referenced_strings.rs` (added in this
commit):
* `reattach_node_contributes_strings_and_handlers` — builds a
  `Patch::Reattach` with a string prop and a handler, asserts the
  walker's output includes both.

## Related

No current production impact; filed as a latent trap.
