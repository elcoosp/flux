# FLUX-F47 — `flux build` silently clobbers generated files on stem / component-name collision

**Status:** OPEN (documented with two regression tests; not fixed — the
fix shape is a design call)
**Found:** round-91 bug hunt (`flux-cli/build.rs` sweep)
**Severity:** low-medium (a project with two `.flux` files that
share a file stem in different directories, or two components that
share the same name, silently loses one of its generated native files
— the user gets a build that compiles fewer components than they
wrote)

## Summary

`crates/flux-cli/src/build.rs::run_with` computes the output path
per-source:

    let stem = path.file_stem()...;
    let root_comp_name = <"App" component, or first>;
    let is_entry = Some(&root_comp_name) == entry_comp_name.as_ref();
    let (code, file_name) = if is_entry {
        ... "<Root>App.swift" / "MainActivity.kt" ...
    } else {
        ... format!("{stem}.{ext}") ...
    };
    let out = out_dir.join(&file_name);
    std::fs::write(&out, code)?;

Two collisions are possible:

1. **Same file stem in different directories.** `src/foo.flux` and
   `lib/foo.flux` both produce `foo.kt` under
   `platforms/android/Generated/`. The second `std::fs::write` clobbers
   the first without warning.

2. **Same component name in two files, matching the global entry name.**
   If `main.flux` and `other.flux` each declare `compo App`, both are
   treated as entry (the global `entry_comp_name` is `"App"`) and both
   write `MainActivity.kt`. Only one survives.

In both cases the build exits 0 and reports success — the user has no
signal that half their source didn't make it into the generated app.

## Regression tests

`crates/flux-cli/src/build.rs::tests`:
* `two_files_same_stem_silently_clobber` — writes both files, drives
  `run_with`, asserts only one `foo.kt` exists (documenting the
  current loss).
* `two_files_same_component_name_silently_clobber` — same shape for
  the `App` name.

Both tests pass today because they assert the *current* lossy
behavior. When a fix lands, the assertions flip to require **both**
outputs.

## What a fix needs

Decide the mapping:

1. **Disambiguate by path.** Emit `Generated/<relative_path_with_underscores>.kt`
   instead of `<stem>.kt`, so `src/foo.flux` → `src__foo.kt` and
   `lib/foo.flux` → `lib__foo.kt`. Deterministic, no collisions in
   practice.

2. **Reject at compile time.** Have the type checker / pipeline
   reject a project where two files define the same component name,
   and where two files share a stem in different directories. This is
   the conservative option: it surfaces the collision as a real error
   the user can fix.

3. **Error only on the entry-path collision.** Keep the current
   stem-based naming for non-entry files (a `foo.kt` per `.flux` is
   what users expect) but error when two sources claim the same output
   file path. This preserves the common case and only fails the
   ambiguous one.

## Related

`docs/issues/FLUX-F17-crash-source-map.md` — the sidecar source map
uses the same `stem` for its filename (`<stem>.flux.map.json`), so it
inherits the same collision property.
