---
title: Cookbook
description: One page per Flux stdlib primitive, cross-linked to its flux doc schema.
---

The cookbook has one page per stdlib component. Each page lists the real prop
contract (mirrored from `flux doc` / `stdlib/*.flux`), a minimal snippet, and a
link to the Todo example that uses it in anger.

The page list below is checked in CI: every `compo` declared in `stdlib/*.flux`
must have a cookbook page, so this list can never silently go stale.

## Components

- [Text](/guides/cookbook/text/) — render a string.
- [Button](/guides/cookbook/button/) — a tappable action.
- [TextInput](/guides/cookbook/textinput/) — a controlled text field.
- [Column](/guides/cookbook/column/) — vertical stack.
- [Row](/guides/cookbook/row/) — horizontal stack.
- [Image](/guides/cookbook/image/) — render a bitmap.
- [Router](/guides/cookbook/router/) — stack navigation.
- [Screen](/guides/cookbook/screen/) — a routed screen.

## How the contract is derived

Each prop table is generated from the component's declaration in `stdlib/`. The
`flux doc` command emits the full JSON schema of the stdlib:

```bash
cargo run --bin flux -- doc
```

That JSON is the source of truth; the cookbook pages are human-readable views of
it. If a prop changes upstream, the CI coverage check (FLUX-031) fails until the
page is updated.
