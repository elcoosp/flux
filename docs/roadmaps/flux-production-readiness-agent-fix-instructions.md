# Flux Codebase — AI Agent Fix Playbook

**Companion document to:** `flux-production-readiness-audit.md` (same folder). The audit explains *why* each task exists; this document tells an executing agent *exactly what to do*, one mechanical step at a time.

**How to read a task:**
- `OLD` blocks show code that currently exists. `NEW` blocks show what replaces it.
- Every task ends with a `VERIFY` command and a `COMMIT` message. A task is only done when VERIFY passes and the commit is made.
- Task IDs (`T-001`…) are stable. Cross-references like `[C7]` point to the finding ID in the audit.

---

## 0. OPERATING PROTOCOL — READ FIRST, FOLLOW LITERALLY

You are an executing agent. You have **no discretion**. Follow these rules without exception.

### R1 — Work in strict order
Execute tasks in the numeric order given in Appendix A, phase by phase (Phase 0 → 1 → 2 → 3 → 4 → 5 → 6). Do not skip ahead. Do not parallelize inside one working copy unless the task says you may.

### R2 — Exact-match editing
Before editing, open the target file and confirm the `OLD` block exists **character-for-character** (allowing only whitespace/line-number drift). If it does not exist:
1. Do NOT guess, do NOT "fix it anyway".
2. Append a row to `PROGRESS.md`: `| <TASK-ID> | BLOCKED | OLD block not found at <file> |`.
3. Move on to the next task.

### R3 — Verify or revert
After each task, run the task's `VERIFY` command(s) in the repository root.
- If VERIFY passes → `git add -A && git commit -m "<task COMMIT message>"`, log `DONE`.
- If VERIFY fails → run `git checkout -- .` (discard), log `FAILED <one-line reason>`, and move on. Never leave the tree in a broken state. Never mark a failed task done.

### R4 — Touch only what the task names
Never edit a file that the task does not list, even if you notice another problem there. Instead, append a row to `PROGRESS.md`: `| SIDECAR | noticed <issue> at <file:line> |`. Sidetracking is how a "dumb" agent breaks a repo.

### R5 — Never do these things
- Never edit `Cargo.toml`, `Package.swift`, `build.gradle.kts`, `settings.gradle.kts`, or `runtimes/ios/project.yml` directly (dependency changes go through `MANIFEST_REQUESTS.md` per repo policy).
- Never delete tests to make them pass. If a test contradicts a task, log `CONFLICT` and skip.
- Never reformat, re-indent, or "clean up" untouched code.
- Never invent API names. If a symbol a task references does not exist, R2 applies (BLOCKED).
- Never push to a remote. Commits only.

### R6 — Baseline before Phase 0
Run all of these and record pass/fail in `PROGRESS.md`. These are your ground truth; a task's VERIFY is judged relative to this baseline.

```bash
cargo build --workspace                         # Rust builds
cargo test --workspace                          # Rust tests (note: flux-differ may fail to compile pre-T-003)
cd runtimes/android && ./gradlew :host:testDebugUnitTest --console=plain   # Kotlin host tests
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | tail -20   # Swift tests
```

If a command's toolchain is not installed, record `TOOLCHAIN MISSING` for that ecosystem and treat its VERIFY steps as "run what is runnable; otherwise static-check only".

### R7 — Progress log
Create `PROGRESS.md` at the repo root before starting, using the template in Appendix C. Update it after every task.

### R8 — Stop-and-ask
A task marked `⚠ STOP-AND-ASK` requires a human decision before you edit. Log `NEEDS DECISION` and move on to tasks that don't depend on it.

### R9 — Meaning of task danger levels
- `LOW` — mechanical, reversible.
- `MEDIUM` — touches behavior; VERIFY gate is mandatory; if the gate is ambiguous, treat as failure (R3).
- `HIGH` — architectural; has explicit sub-steps and a stop-condition. Do not improvise beyond the steps.

---

## 1. REPOSITORY MAP

All paths are relative to the repo root (`/home/z/my-project/codebase/` when working locally).

| Area | Path |
|---|---|
| Rust workspace | `crates/` (16 crates; key: `flux-ir`, `flux-ir-serde`, `flux-differ`, `flux-devserver`, `flux-parser`, `flux-types`, `flux-vm-ref`, `flux-parity`, `flux-codegen-core`, `flux-codegen-kotlin`, `flux-codegen-swift`, `flux-cli`, `flux-lsp`) |
| Kotlin host runtime | `runtimes/android/host/src/main/kotlin/dev/flux/host/` |
| Kotlin app shell | `runtimes/android/app/src/main/kotlin/dev/flux/app/` |
| Kotlin adapter kit | `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/` |
| Swift host runtime | `runtimes/ios/FluxHost/Sources/FluxHost/` |
| Swift adapter kit | `adapters/ui-swift/Sources/FluxUIKit/` |
| Stdlib (.flux) | `stdlib/*.flux` |
| CI workflows | `.github/workflows/*.yml` (16 files) |
| Scripts | `scripts/` |
| ISA/vector/fixture tests | `tests/isa-vectors/` (currently **absent** — several tasks create it), `fixtures/wire/` (currently README only) |

**Verification command cheat sheet** (full table in Appendix B):

| Subsystem | Command |
|---|---|
| One Rust crate | `cargo test -p <crate>` |
| Whole Rust workspace | `cargo test --workspace` |
| Kotlin host | `cd runtimes/android && ./gradlew :host:testDebugUnitTest --console=plain` |
| Swift host | `cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS'` |
| Swift adapter kit | `cd adapters/ui-swift && xcodebuild test -scheme FluxUIKit -destination 'platform=macOS'` |
| Stdlib parse gate | `bash scripts/parse-check.sh` |

---

# PHASE 0 — UNBLOCK CI AND TOOLING

**Goal:** every existing workflow and test command can actually run. Nothing here changes runtime behavior.
**Exit criteria:** `cargo test -p flux-differ` compiles; `check-contract-freeze.sh` runs to a verdict; `ci-size-gate.sh` detects force-unwraps; workflows declare permissions/concurrency.

---

### T-001 — Create the missing contract-freeze manifest [C13] — Danger: LOW

**Files:** `docs/release/contract-versions.toml` (create; directory `docs/release/` does not exist yet)

1. Determine the frozen wire version (it must equal what all three decoders already implement):
   ```bash
   grep -oE 'pub const PROTOCOL_VERSION: u8 = [0-9]+' crates/flux-ir-serde/src/frame.rs
   ```
   Note the integer (expected `2`).
2. Determine the frozen adapter contract versions:
   ```bash
   grep -oE 'adapterContractVersion = [0-9]+' adapters/ui-swift/Sources/FluxUIKit/FluxUIKit.swift
   grep -oE 'ADAPTER_CONTRACT_VERSION: Int = [0-9]+' adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxUiKit.kt
   ```
   If the two differ, use the **larger** number and append a `SIDECAR` row noting the mismatch (a later task, T-607, reconciles them).
3. Create `docs/release/` and write this file exactly (substitute the values you found):

```toml
# Frozen release contract versions (checked by scripts/release-gate/check-contract-freeze.sh).
# Bump only via a signed release ticket; the release gate fails any mismatch.
wire = 2          # must equal PROTOCOL_VERSION in crates/flux-ir-serde/src/frame.rs,
                  # FrameDeserializer.kt SUPPORTED_VERSIONS, and FrameDeserializer.swift
adapter = 1       # must equal adapterContractVersion (Swift) == ADAPTER_CONTRACT_VERSION (Kotlin)
pin = ""          # optional: set to a release tag (e.g. "v1.0.0") to pin the gate to that ref
```

**VERIFY**
```bash
bash scripts/release-gate/check-contract-freeze.sh; echo "exit=$?"
```
Expected: lines beginning `[ok]` for Rust/Android/iOS wire + both adapter versions, final line `contract freeze check: PASS`, `exit=0`.

**COMMIT:** `fix(C13): commit contract-versions.toml so the release gate can run`

---

### T-002 — Fix the broken Gradle wrapper in CI [C13] — Danger: LOW

**Files:** `gradle/wrapper/` (create one file), or three workflow files (fallback path)

`gradle/wrapper/` contains only `gradle-wrapper.properties`; `gradle-wrapper.jar` is missing, so every `./gradlew` invocation fails.

**Path A (preferred — if `gradle` is installed locally):**
1. Read the pinned version:
   ```bash
   grep distributionUrl gradle/wrapper/gradle-wrapper.properties
   ```
2. From the repo root run:
   ```bash
   gradle wrapper --gradle-version <version-from-step-1>
   ```
3. Confirm `gradle/wrapper/gradle-wrapper.jar` now exists (`ls -la gradle/wrapper/`).
4. Add the jar to git if a `.gitignore` rule excludes `*.jar` under `gradle/` — check with `git check-ignore -v gradle/wrapper/gradle-wrapper.jar`; if ignored, add this line to `.gitignore` at the END of the file: `!gradle/wrapper/gradle-wrapper.jar`.

**Path B (fallback — no local gradle):** edit the three workflows `.github/workflows/android-check.yml`, `compat-matrix.yml`, `artifact-publish.yml`. In each, immediately before the first step that invokes `./gradlew`, insert:

```yaml
      - name: Set up Gradle
        uses: gradle/actions/setup-gradle@v4
        with:
          cache-read-only: false
```

and replace every occurrence of `./gradlew` in that workflow with `gradle`.

**VERIFY**
```bash
test -f gradle/wrapper/gradle-wrapper.jar && echo JAR-OK || echo PATH-B
grep -n "setup-gradle" .github/workflows/android-check.yml || true
```
Expected: either `JAR-OK`, or `PATH-B` plus at least one `setup-gradle` hit in each of the three workflows.

**COMMIT:** `fix(C13): restore executable Gradle wrapper for CI (jar or setup-gradle)`

---

### T-003 — Restore a compiling differ test crate [H13] — Danger: LOW

**Files:** `crates/flux-differ/src/diff/tests.rs`

The file declares three test modules whose files do not exist, so `cargo test -p flux-differ` cannot compile. Delete the declarations (the audit found no remaining test content in this crate to preserve).

1. Replace the entire contents of `crates/flux-differ/src/diff/tests.rs` with:

```rust
//! Differ golden tests. The historical `common` / `patch_tests` /
//! `reattach_tests` modules were never committed (audit H13), which broke the
//! crate's build. Rebuilt in Phase 2 (T-212, T-213) as inline `#[cfg(test)]`
//! modules next to the code they cover.
```

**VERIFY**
```bash
cargo test -p flux-differ 2>&1 | tail -5
```
Expected: compilation succeeds (`test result:` line present, zero tests is acceptable at this point).

**COMMIT:** `fix(H13): remove phantom test-module declarations so flux-differ compiles`

---

### T-004 — Repair the force-unwrap size-gate regex [C14] — Danger: LOW

**Files:** `scripts/ci-size-gate.sh`

The regex `[A-Za-z0-9_)\]]` is a POSIX bracket expression; the `\]` inside it does **not** mean "literal `]`" — it closes the class early and turns `]` into a required literal character. Result: `x!` never matches and the Swift/Kotlin force-unwrap rule never fires. The replacement below was tested against `x!`, `b! + 1`, `arr[i]!`, `(n)!`, `try!`, and correctly rejects `w != q`.

There are **two** occurrences to replace (line ~240 grep version, line ~245 count version).

**Occurrence 1** — find:

```bash
          done < <(grep -nE '(try!|[A-Za-z0-9_)\]]\s*!(=|\?|;|,|\)|\s|$))' "$f" \
                     | grep -vE '//.*(!|\?)' | grep -E '(!|\?)')
```

replace with:

```bash
          done < <(grep -nE '(try!|[]A-Za-z0-9_)]!([^=]|$))' "$f" \
                     | grep -vE '//.*(!|\?)' | grep -E '(!|\?)')
```

**Occurrence 2** — find:

```bash
            | grep -cE '(try!|[A-Za-z0-9_)\]]\s*!(=|\?|;|,|\)|\s|$))')"
```

replace with:

```bash
            | grep -cE '(try!|[]A-Za-z0-9_)]!([^=]|$))')"
```

**VERIFY**
```bash
printf 'let y = x!\nlet a = b! + 1\nlet z = w != q\nlet k = arr[i]!\nlet m = (n)!\nlet t = try! foo()\n' > /tmp/sg.txt
grep -cE '(try!|[]A-Za-z0-9_)]!([^=]|$))' /tmp/sg.txt
```
Expected output: `5` (matches the five force-unwrap/try lines, not the `!=` lines).

**COMMIT:** `fix(C14): repair force-unwrap regex in ci-size-gate.sh (bracket-class bug)`

---

### T-005 — Clear the stale manifest request and make the steward self-checking [C14] — Danger: LOW

**Files:** `MANIFEST_REQUESTS.md`, `scripts/manifest-steward.sh`

The open row for `flux-devtools-ui → flux-perf-harness` was already applied directly to `Cargo.toml` (verify: `grep -n "flux-perf-harness" crates/flux-devtools-ui/Cargo.toml` must print a hit). The steward's regex does not recognize the `*.workspace = true` form, so the next weekly cron run would re-insert the dependency as invalid TOML and break the workspace build.

1. In `MANIFEST_REQUESTS.md`, delete the table row beginning `| flux-devtools-ui | flux-perf-harness |` (line ~18). Leave the explanatory blockquote below it intact.
2. In `scripts/manifest-steward.sh`, immediately after the point where the script finishes applying a request and before it commits (locate the commit step with `grep -n "git commit" scripts/manifest-steward.sh`), insert this validation block:

```bash
  # Validate the workspace still parses as TOML before committing (audit C14).
  # A bad insertion must fail the steward PR, not break main.
  if command -v cargo >/dev/null 2>&1; then
    if ! cargo metadata --no-deps --format-version 1 >/dev/null 2> "$STEWARD_TMP_ERR"; then
      echo "::error::manifest edit produced an invalid workspace TOML; aborting" >&2
      cat "$STEWARD_TMP_ERR" >&2
      exit 1
    fi
  elif command -v python3 >/dev/null 2>&1; then
    python3 - <<'PYTOML' || { echo "::error::manifest edit produced invalid TOML" >&2; exit 1; }
import tomllib, sys
for p in ("Cargo.toml", "crates/flux-devtools-ui/Cargo.toml"):
    with open(p, "rb") as fh:
        tomllib.load(fh)
PYTOML
  fi
```

3. At the top of the same script, where other temp variables are initialized (search `TMPDIR` or the first variable block), add:

```bash
STEWARD_TMP_ERR="$(mktemp)"
trap 'rm -f "$STEWARD_TMP_ERR"' EXIT
```

**VERIFY**
```bash
bash -n scripts/manifest-steward.sh && echo SYNTAX-OK
grep -c "flux-devtools-ui | flux-perf-harness" MANIFEST_REQUESTS.md || true
```
Expected: `SYNTAX-OK` and `0` (the row is gone). Do not run the steward itself outside `--dry-run` mode.

**COMMIT:** `fix(C14): drop stale manifest request row; steward validates TOML before committing`

---

### T-006 — Un-ignore the fuzz corpus and commit seeds [H28] — Danger: LOW

**Files:** `.gitignore`, `fuzz/corpus/**` (create)

`.gitignore` line ~43 contains `fuzz/corpus`, but `wire-fuzz.yml` requires a committed corpus. Un-ignore it and commit minimal seeds.

1. In `.gitignore`, delete the line containing exactly `fuzz/corpus`.
2. Create the seed directory and one seed per fuzz target that exists under `fuzz/`:
   ```bash
   mkdir -p fuzz/corpus/decode_frame
   ```
   List existing targets: `ls fuzz/fuzz_targets/ 2>/dev/null || ls fuzz/` — for every target name found, `mkdir -p fuzz/corpus/<name>`.
3. For `decode_frame`, write one valid seed by copying the known-good Init frame magic + version header (4 bytes magic `46 5C 55 58` + 1 byte version `02`):
   ```bash
   printf '\x46\x5c\x55\x58\x02' > fuzz/corpus/decode_frame/seed_header_v2.bin
   printf '\x00\x00\x00\x00\x00'   > fuzz/corpus/decode_frame/seed_empty.bin
   ```
   For other targets, add a single 16-byte zero seed file each (fuzzers accept any bytes): `printf '\x00%.0s' {1..16} > fuzz/corpus/<target>/seed16.bin`.

**VERIFY**
```bash
git check-ignore fuzz/corpus/decode_frame/seed_header_v2.bin; echo "ignored=$?"
ls fuzz/corpus/*/ | head
```
Expected: `ignored=1` (not ignored) and the seed files listed.

**COMMIT:** `fix(H28): track fuzz corpus with minimal seeds (wire-fuzz workflow requirement)`

---

### T-007 — Delete the dead ForEach hash generator script [H30] — Danger: LOW

**Files:** `scripts/generate_for_each_hashes.sh` (delete)

The script writes two blake3 constants to a file with zero consumers, matches neither host's actual id derivation, and lives under a machine-specific `${HOME}/.hermes/scripts` path.

1. Confirm zero consumers:
   ```bash
   grep -rn "for_each_hashes\|generate_for_each_hashes" --include="*.rs" --include="*.kt" --include="*.swift" --include="*.yml" --include="*.sh" . | grep -v "scripts/generate_for_each_hashes.sh"
   ```
   Expected: no output. If there IS output, log `BLOCKED` (R2) — do not delete.
2. `git rm scripts/generate_for_each_hashes.sh`

**VERIFY**
```bash
test ! -f scripts/generate_for_each_hashes.sh && echo DELETED
```

**COMMIT:** `chore(H30): remove dead generate_for_each_hashes.sh (zero consumers, stale constants)`

---

### T-008 — Workflow hardening sweep: permissions, concurrency, SHA pinning — Danger: MEDIUM

**Files:** all 16 files in `.github/workflows/`

For **each** `*.yml` file in `.github/workflows/`:

1. If the file has no top-level `permissions:` key, insert directly under the top-level `on:` block's closing line (or after the `name:` line when `on:` uses inline form):

```yaml
permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: false
```

   Exception: workflows that already need write scope (`artifact-publish.yml`, `release-gate.yml`) use `contents: write` instead of `contents: read`.

2. For every `uses: <action>@<tag-or-branch>` line where the tag is a **version tag** (e.g. `@v4`, `@v5`) and the action is third-party (anything NOT starting with `actions/checkout`'s owner `actions`, `gradle`, `docker`, or `github`), leave it in place but log it. Do not attempt to resolve SHAs offline — instead add this guard comment directly above the line:

```yaml
      # TODO(sha-pin): resolve the commit SHA for this third-party action before v1.0
```

   (`actions/checkout@v4`, `actions/upload-artifact@v4`, `gradle/actions/*`, `docker/*`, `github/codeql-action/*` may remain tag-pinned without the comment.)

3. In `benchmarks.yml`: find the condition that skips benches on PRs (search `if:` near `cargo bench`) and change it so benches also run on PRs touching `crates/` — replace `github.event_name != 'pull_request'` with `true` ONLY on the bench step, and add the comment `# release-gate consumes this job's result; skipping would fake a green gate`.

**VERIFY**
```bash
for f in .github/workflows/*.yml; do
  grep -q "^permissions:" "$f" || echo "MISSING permissions: $f"
  grep -q "^concurrency:" "$f" || echo "MISSING concurrency: $f"
done
```
Expected: no output.

**COMMIT:** `ci: add permissions+concurrency to all workflows; flag unpinned third-party actions`

---

### T-009 — Phase 0 exit check

Run in order; all must pass (or be recorded as `TOOLCHAIN MISSING` from R6):

```bash
cargo test -p flux-differ 2>&1 | tail -2
bash scripts/release-gate/check-contract-freeze.sh >/dev/null; echo "gate=$?"
bash scripts/ci-size-gate.sh --help >/dev/null 2>&1 || bash -n scripts/ci-size-gate.sh && echo "size-gate syntax ok"
for f in .github/workflows/*.yml; do grep -q "^permissions:" "$f" || echo "BAD $f"; done
```

Update `PROGRESS.md` with the Phase 0 table, then proceed to Phase 1.

# PHASE 1 — LANGUAGE CORRECTNESS: THE COMPILER MUST NOT LIE

**Goal:** eliminate every silent miscompilation. After this phase, any program the compiler accepts either runs correctly or fails the build loudly.
**Exit criteria:** new conformance tests below pass; `cargo test --workspace` green.
**Primary file:** `crates/flux-ir/src/lower/bytecode.rs` (the emitter), `crates/flux-ir/src/lower/mod.rs` (lowering), `crates/flux-ir/src/lower/ids.rs` (NodeIds).

---

### T-101 — Register allocator must fail loudly instead of aliasing live registers [C1] — Danger: MEDIUM

**Files:** `crates/flux-ir/src/lower/bytecode.rs`

Today `alloc_reg` saturates at register 14, so once a handler needs ~13+ temporaries every later allocation silently returns `r14` while older values are still live — arithmetic becomes garbage. Step 1 makes exhaustion loud; step 2 (T-102) makes exhaustion rare.

**Step 1 — change the signature and thread the error.**

1. Replace:

```rust
    fn alloc_reg(&mut self) -> u8 {
        let r = self.reg;
        self.reg = self.reg.saturating_add(1).min(14);
        r
    }
```

with:

```rust
    /// Allocates the next scratch register. Handlers have 15 registers (r0..r14);
    /// exhausting them is a compile error, never a silent alias of a live
    /// register (audit C1 — a saturating allocator made `count = count + 1`
    /// miscompile whenever a handler needed too many temporaries).
    fn alloc_reg(&mut self) -> Result<u8, HandlerCompileError> {
        if self.reg > 14 {
            return Err(HandlerCompileError::new(
                "handler uses more than 15 registers — simplify the expression \
                 (register pressure limit, audit C1)"
                    .to_owned(),
                self.current_span(),
            ));
        }
        let r = self.reg;
        self.reg += 1;
        Ok(r)
    }

    /// The span of the expression currently being compiled; used for the
    /// register-exhaustion diagnostic. If the emitter has no span field,
    /// thread the enclosing expression's span into `alloc_reg` calls instead.
    fn current_span(&self) -> flux_syntax::Span {
        self.span
    }
```

   Note: if `Emitter` has no `span` field, add `span: flux_syntax::Span` to the struct, initialize it wherever the emitter is constructed (use `Span::new(0,0,0)` initially), and set it at the top of `compile_value`/`compile_call` from the expression's `expr.span`. Search first: `grep -n "struct Emitter" -A 20 crates/flux-ir/src/lower/bytecode.rs`.

2. Mechanically thread the `Result` through every caller **inside `bytecode.rs` only**:
   ```bash
   grep -n "self.alloc_reg()" crates/flux-ir/src/lower/bytecode.rs
   ```
   For each hit, change `self.alloc_reg()` to `self.alloc_reg()?` **only where the enclosing function already returns `Result<_, HandlerCompileError>`** (all `compile_*` functions do). For calls inside `fn`s that do NOT return `Result` (e.g. some `emit_*` helpers), leave them and note each one; then convert those helper signatures to return `Result<(), HandlerCompileError>` as cargo's errors direct. Iterate `cargo build -p flux-ir` until clean. Do not change any other file.

**Step 2 — regression test.** Append to the `#[cfg(test)]` module at the bottom of `bytecode.rs` (create it if absent: search `mod tests`):

```rust
    #[test]
    fn twenty_sequential_statements_get_distinct_live_registers() {
        // Compile a handler that performs 20 sequential signal updates
        // (e.g. `count = count + 1` written out 20 times against distinct
        // signals s0..s19). Before C1 this silently aliased r14; now it must
        // either compile with correct per-statement registers (with T-102's
        // watermark reuse) or fail loudly with RegistersExhausted.
        let src = build_handler_with_n_sequential_increments(20); // helper below
        let result = compile_handler_for_test(&src);
        match result {
            Ok(code) => {
                // Every READ_SIGNAL must target a register whose next write
                // does not clobber it before its ADD. Cheap invariant: the
                // register operand of ADD equals the READ_SIGNAL dst, and the
                // const-load between them targets a DIFFERENT register.
                assert_no_clobbered_const_loads(&code);
            }
            Err(e) => panic!("handler must compile under 15 regs with watermark reuse; got: {e}"),
        }
    }
```

   The two helper functions may not exist yet — if not, write them next to the test: `build_handler_with_n_sequential_increments(n)` produces the `.flux` handler source string with `n` statements `sN = sN + 1` (signals pre-declared), and `assert_no_clobbered_const_loads` scans the emitted bytes: for each `LOAD_*_CONST rX` at index i, no later `ADD rD, rX, rX` may appear where the const was the intended summand AND `rX` was also the prior `READ_SIGNAL` dst between them. If implementing the scanner is beyond scope, assert the weaker invariant: `Err` is never the silent case — i.e. when `self.reg` would exceed 14 the compiler returns the register-pressure error string above.

**VERIFY**
```bash
cargo build -p flux-ir 2>&1 | tail -3
cargo test -p flux-ir bytecode 2>&1 | tail -5
```

**COMMIT:** `fix(C1): register allocator returns a compile error instead of aliasing live registers`

---

### T-102 — Statement-scope register watermark (make exhaustion rare) — Danger: HIGH ⚠ verify-then-keep-or-revert

**Files:** `crates/flux-ir/src/lower/bytecode.rs`

**Precondition:** T-101 merged and green. If T-101 was BLOCKED, skip this task.

Scratch registers only need to live for the duration of one statement (signal cells, records, and props persist outside registers). Give the emitter a watermark API and reset it at statement boundaries.

1. Add to `Emitter`:

```rust
    /// Statement-scope watermark: registers at or above the watermark are
    /// scratch for the current statement and are reclaimed when it ends.
    /// Signal cells / records / props persist outside registers, so nothing
    /// live crosses a statement boundary (audit C1 step 2).
    watermark: u8,
```

   Initialize `watermark: 0` at construction.
2. Add methods:

```rust
    fn push_watermark(&mut self) { self.watermark = self.reg; }
    fn pop_watermark(&mut self) { self.reg = self.watermark; }
```

3. Find where handler statements are iterated (search `for stmt in` inside the handler-compilation function in this file, and the statement match in `crates/flux-ir/src/lower/mod.rs` if the loop lives there — check with `grep -n "Statement" crates/flux-ir/src/lower/bytecode.rs | head`). Wrap each statement's compilation:

```rust
self.push_watermark();
let outcome = /* existing per-statement compile, unchanged */;
self.pop_watermark();
```

4. Run the FULL workspace test suite. If any previously-green test now fails, revert this task entirely (`git checkout -- crates/flux-ir`) and log `T-102 REVERTED (kept T-101 only)` — T-101 alone is still an acceptable end state (loud failure instead of silent corruption).

**VERIFY**
```bash
cargo test --workspace 2>&1 | tail -5
```
Plus re-run T-101's regression test — it must now take the `Ok` path.

**COMMIT:** `perf(C1): reclaim statement-scope scratch registers via watermark`

---

### T-103 — Select opcodes from the type checker's recorded types, not syntax [C2] — Danger: HIGH

**Files:** `crates/flux-ir/src/lower/bytecode.rs`, `crates/flux-ir/src/lower/mod.rs` (threading only)

Today every arithmetic/comparison emits the I64 opcode family regardless of operand type, so `total = total + 0.5` faults with `ADD_I64` on a Float and `name == "bob"` faults on Str. The correct types already exist in the type checker's map keyed by expression NodeId (the bridge is `crates/flux-codegen-core/src/bridge.rs`).

**Step 1 — discover the type map's exact name and shape:**
```bash
grep -n "typed.types\|pub struct Typed\|types:" crates/flux-codegen-core/src/bridge.rs | head -20
grep -n "fn type_of\|expr_types\|node_types" crates/flux-ir/src/lower/*.rs | head
```
Identify: (a) the map type (e.g. `FxHashMap<NodeId, Ty>`), (b) the `Ty` enum variant names for Float/Str/Int/Bool (grep `enum Ty` or `pub enum Type` in `crates/flux-types/src/`).

**Step 2 — thread the map into the emitter.** In `lower/mod.rs`, wherever the handler emitter (`Emitter`/`HandlerCompiler`) is constructed, add a field `expr_types: <the map type>` populated from the typed AST that lowering already receives. If lowering does NOT currently receive it, stop, log `BLOCKED: no type map at lowering input`, and skip to T-104 (do not redesign the pipeline).

**Step 3 — typed opcode selection.** In `compile_value`'s `Binary` arm, locate (current code, verified at lines 1187–1219):

```rust
                let opcode = match op {
                    BinOp::Add => raw::ADD_I64,
                    BinOp::Sub => raw::SUB_I64,
                    BinOp::Mul => raw::MUL_I64,
                    BinOp::Div => raw::DIV_I64,
                    BinOp::Rem => raw::MOD_I64,
```

Replace the whole `let opcode = match op { ... };` block (through the `Ge`/`And`/`Or` arms) with:

```rust
                // Typed opcode selection (audit C2): the operand type recorded
                // by the type checker decides the opcode family. The previous
                // syntactic heuristic emitted I64 ops for Float/Str operands,
                // which faults at runtime.
                let operand_ty = self
                    .expr_types
                    .get(&expr_node_id_of(expr))
                    .copied()
                    .unwrap_or(Ty::Unknown);
                use Ty as T;
                let opcode = match op {
                    BinOp::Add => match operand_ty {
                        T::Float => raw::ADD_F64,
                        T::Str => raw::STR_CONCAT,
                        _ => raw::ADD_I64,
                    },
                    BinOp::Sub => match operand_ty {
                        T::Float => raw::SUB_F64,
                        _ => raw::SUB_I64,
                    },
                    BinOp::Mul => match operand_ty {
                        T::Float => raw::MUL_F64,
                        _ => raw::MUL_I64,
                    },
                    BinOp::Div => match operand_ty {
                        T::Float => raw::DIV_F64,
                        _ => raw::DIV_I64,
                    },
                    BinOp::Rem => raw::MOD_I64,
                    BinOp::Eq | BinOp::Ne => match operand_ty {
                        T::Float => raw::EQ_F64,
                        T::Str => raw::STR_EQ,
                        T::Bool => raw::BOOL_EQ,
                        _ => raw::EQ_I64,
                    },
                    BinOp::Lt => match operand_ty {
                        T::Float => raw::LT_F64,
                        _ => raw::LT_I64,
                    },
                    BinOp::Gt => match operand_ty {
                        T::Float => raw::GT_F64,
                        _ => raw::GT_I64,
                    },
                    BinOp::Le => match operand_ty {
                        T::Float => raw::LTE_F64,
                        _ => raw::LTE_I64,
                    },
                    BinOp::Ge => match operand_ty {
                        T::Float => raw::GTE_F64,
                        _ => raw::GTE_I64,
                    },
                    BinOp::And => raw::AND_BOOL,
                    BinOp::Or => raw::OR_BOOL,
                    _ => {
                        return Err(HandlerCompileError::new(
                            "unsupported operator in handler".to_owned(),
                            expr.span,
                        ));
                    }
                };
```

   Adapt mechanically, per R2/R4: `Ty::Unknown` / the exact variant names / `expr_node_id_of(expr)` must be replaced by the real symbols discovered in Step 1 (`expr_node_id` from `ids.rs` is the NodeId derivation — it takes `&Expr`). If `LTE_F64`/`GTE_F64` do not exist in `raw`, check `crates/flux-ir/src/lower/raw.rs` (or wherever `raw::` is defined) for the real F64 comparison names and use those; if Float `<=`/`>=` opcodes genuinely do not exist in the ISA, emit `LT_F64`/`GT_F64` and log a `SIDECAR` row noting the ISA gap.
   Also **delete the now-dead `is_bool_eq` heuristic block** immediately above the match (lines ~1180–1186, the `matches!(lhs.kind, …) && matches!(rhs.kind, …)` computation) — it is replaced by the typed path.

**Step 4 — conformance vectors.** Add `tests/isa-vectors/typed_arith.json` (create directories) with cases:

```json
{
  "cases": [
    { "name": "float_add",   "src": "total = total + 0.5", "op": "ADD_F64" },
    { "name": "str_eq",      "src": "ok = name == \"bob\"", "op": "STR_EQ" },
    { "name": "int_add",     "src": "count = count + 1",   "op": "ADD_I64" },
    { "name": "bool_eq",     "src": "flag = ready == true","op": "BOOL_EQ" }
  ]
}
```

   and a Rust test `#[test] fn typed_opcode_selection_matches_vectors()` in `bytecode.rs`'s test module that compiles each `src` and asserts the named opcode byte appears at the arithmetic site. (The JSON is the durable artifact; later phases reference the directory.)

**VERIFY**
```bash
cargo test -p flux-ir 2>&1 | tail -5
cargo test --workspace 2>&1 | grep -E "test result" | tail -5
```

**COMMIT:** `fix(C2): emit Float/Str/Bool opcodes from checker-recorded types (typed arithmetic)`

---

### T-104 — `?.` must read the same field-index space as `.` and SET_FIELD [C3] — Danger: LOW

**Files:** `crates/flux-ir/src/lower/bytecode.rs`

`GET_FIELD` for the null-safe chain uses `method_id_for("", field)` (a blake3-derived u16), while every other field access uses `prop_index_for_name` (FNV u16). `user?.name` therefore reads a different slot than `user.name`.

Find (verified at lines 1274–1277):

```rust
                self.code.push(raw::GET_FIELD);
                self.code.push(out);
                let tag = method_id_for("", &field.name);
                self.code.extend_from_slice(&tag.to_le_bytes());
```

Replace with:

```rust
                self.code.push(raw::GET_FIELD);
                self.code.push(out);
                // Same index space as every other field access (audit C3):
                // `?.` previously hashed the field name with blake3 via
                // method_id_for, reading a slot nothing ever wrote.
                let tag = prop_index_for_name(&field.name);
                self.code.extend_from_slice(&tag.to_le_bytes());
```

Add the regression test to the test module:

```rust
    #[test]
    fn opt_field_uses_same_index_as_plain_field() {
        // `user?.name` and `user.name` must emit the identical GET_FIELD index.
        let plain = compile_handler_for_test("x = user.name");
        let opt = compile_handler_for_test("x = user?.name");
        let plain_idx = first_get_field_index(&plain);
        let opt_idx = first_get_field_index(&opt);
        assert_eq!(plain_idx, opt_idx, "?. and . must agree on the field slot");
    }
```

   (`first_get_field_index` scans the bytecode for the `GET_FIELD` opcode byte and returns the following u16 LE. Reuse the test helpers from T-101.)

**VERIFY**
```bash
cargo test -p flux-ir opt_field 2>&1 | tail -3 && cargo test --workspace 2>&1 | grep -c "test result: ok"
```

**COMMIT:** `fix(C3): null-safe field access reads prop_index_for_name slot space`

---

### T-105 — Literal `match` arms must compare values, not just type tags [C4] — Danger: HIGH

**Files:** `crates/flux-ir/src/lower/bytecode.rs`

`match n { 1 => A, 2 => B, _ => C }` compiles every literal arm to `MATCH_TAG` carrying only the *type* tag (see `match_tag_for_pattern`, lines 967–980), so every Int matches the first arm. Implement value equality for literal arms.

**Step 1 — locate the match compiler:** `grep -n "fn compile_match" crates/flux-ir/src/lower/bytecode.rs`. Understand its per-arm emission: it currently emits `MATCH_TAG scrutinee, tag` + `COND_JUMP_NOT next_arm` + arm body.

**Step 2 — new emission per literal arm.** For an arm whose pattern is `Literal(lit)`, emit this sequence instead of a bare MATCH_TAG jump:

```
MATCH_TAG   scratch, <type tag of lit>      ; cheap type gate (existing op)
COND_JUMP_NOT next_arm                      ; wrong type -> next arm
<LOAD opcode for lit value> scratch2        ; LOAD_INT_CONST / LOAD_BOOL_CONST /
                                            ; LOAD_FLOAT_CONST / LOAD_STR_CONST
EQ_OP       scratch, scratch2, scrutinee    ; EQ_I64 / BOOL_EQ / EQ_F64 / STR_EQ
COND_JUMP_NOT next_arm                      ; wrong value -> next arm
<arm body>
```

   Mechanically: extend `match_tag_for_pattern` to return an enum `ArmTest { Tag(u32), TagAndValue { tag: u32, value: Value } }`; in `compile_match`, when a literal arm yields `TagAndValue`, emit the four extra instructions between the MATCH_TAG jump and the body. Choose the EQ opcode from the literal's type (mirror T-103's mapping). Reuse the existing `jump_placeholder`/`patch_jump` helpers for `next_arm`.
   If the existing `compile_match` structure cannot express "two chained tests per arm" without a rewrite, STOP per R5, log `T-105 NEEDS REDESIGN`, and instead implement the fallback: make `match_tag_for_pattern` return an error for `Literal` patterns ("literal match arms in handlers are not yet supported — use variant or wildcard arms (audit C4)"), which is loud rather than wrong. Then also add the same rejection to `crates/flux-types/src/exhaust.rs` so the type checker surfaces it. Log which path you took.

**Step 3 — test:**

```rust
    #[test]
    fn literal_match_arms_are_value_sensitive() {
        // match n { 1 => A(), 2 => B(), _ => C() } with n = 2 must run B's body.
        let code = compile_handler_for_test(
            "match n { 1 => { a = 1 }, 2 => { a = 2 }, _ => { a = 0 } }"
        );
        // Execute against flux-vm-ref with n = 2 and assert signal a == 2.
        let outcome = run_in_vm_ref(&code, &[("n", Value::Int(2))]);
        assert_eq!(outcome.read_signal("a"), Value::Int(2));
    }
```

**VERIFY**
```bash
cargo test -p flux-ir literal_match 2>&1 | tail -3
cargo test --workspace 2>&1 | grep -E "test result" | tail -3
```

**COMMIT:** `fix(C4): literal match arms compare values (MATCH_TAG + equality), not type tags alone`

---

### T-106 — Handler-constructed enum variants must carry their tag [C5] — Danger: MEDIUM

**Files:** `crates/flux-ir/src/lower/bytecode.rs`, `crates/flux-ir/src/lower/mod.rs`

The VM reads a variant's match tag from the record's field 0 (`crates/flux-vm-ref/src/vm.rs:715-723`), but records built inside a handler (and static seeds) store only the named payload fields. `match` over a handler-constructed `Task(...)` therefore never matches.

**Site 1 — handler construction.** In `compile_call`'s `ExprKind::Ident(ident)` constructor branch (verified lines 1435–1449), find:

```rust
                    let dst = self.alloc_reg();
                    self.emit_alloc_record(dst, arg_regs.len() as u16);
                    let mut positional: u16 = 0;
```

Replace with:

```rust
                    let dst = self.alloc_reg()?;
                    // Field 0 carries the variant tag (audit C5): the VM's
                    // MATCH_TAG reads tag from record field 0. +1 for the tag
                    // slot; reject a payload field whose PropIdx collides with 0.
                    self.emit_alloc_record(dst, arg_regs.len() as u16 + 1);
                    let tag_reg = self.alloc_reg()?;
                    self.emit_load_int_const(tag_reg, variant_tag(&ident.name) as i64);
                    self.emit_set_field(dst, 0, tag_reg);
                    let mut positional: u16 = 0;
```

   Adapt: if `emit_load_int_const` is named differently (search `fn emit_load` in the file), use the real name; the tag is stored as an `Int` value. Additionally, inside the per-arg loop below, if a **named** arg's `prop_index_for_name(&name.name)` computes to `0`, return a compile error: `"record field hashes to the reserved tag slot 0 — rename the field (audit C5)"`.

**Site 2 — static seed.** In `crates/flux-ir/src/lower/mod.rs` (verified lines 883–894), find the `Record { name: _, fields }` arm:

```rust
                let mut lowered: Vec<(flux_syntax::PropIdx, Value)> =
                    Vec::with_capacity(fields.len());
                for (fname, fexpr) in fields {
                    let idx = prop_index_for_name(&fname.name);
                    lowered.push((idx, self.lower_value(fexpr, owner, handlers)?));
                }
                Ok(Value::Record(lowered))
```

Replace with:

```rust
                // Field 0 carries the variant tag so `match` over a seeded
                // record agrees with handler construction (audit C5).
                let mut lowered: Vec<(flux_syntax::PropIdx, Value)> =
                    Vec::with_capacity(fields.len() + 1);
                lowered.push((
                    flux_syntax::PropIdx::from(0u16),
                    Value::Int(i64::from(variant_tag(&record_type_name(&fields)))),
                ));
                for (fname, fexpr) in fields {
                    let idx = prop_index_for_name(&fname.name);
                    if idx == flux_syntax::PropIdx::from(0u16) {
                        return Err(/* the same reserved-slot error as site 1 */);
                    }
                    lowered.push((idx, self.lower_value(fexpr, owner, handlers)?));
                }
                Ok(Value::Record(lowered))
```

   Mechanically: this arm does not currently know the record's *type name* — the pattern binds `name: _`. Change the binding to `name` and use it for the tag (`variant_tag(name)`). If the enclosing code cannot access the name, log `BLOCKED: record literal has no type name at this site` and only fix site 1 + the VM-side consumption check below.

**Site 3 — unmask the test.** In the existing unit test that hand-seeds `Record([(0, Int(tag))])` (search `grep -rn "0, Int(" crates/flux-ir/src/ crates/flux-vm-ref/src/`), remove the hand-seeded tag field so the test now exercises the real construction path. If the test then fails, the fix above is incomplete — fix, do not re-seed.

**VERIFY**
```bash
cargo test -p flux-ir 2>&1 | tail -3
cargo test -p flux-vm-ref 2>&1 | tail -3
```

**COMMIT:** `fix(C5): variant tag stored in record field 0 on both construction paths`

---

### T-107 — Component inlining must not mint duplicate NodeIds [C6] — Danger: HIGH

**Files:** `crates/flux-ir/src/lower/ids.rs`, `crates/flux-ir/src/lower/mod.rs`

`try_inline_component` re-lowers a component body per call site; `expr_node_id` derives ids only from the body's own spans (`ids.rs:66-68`, `parent = 0`), so two call sites of the same component produce byte-identical NodeIds, and `arena.rs` silently keeps the last. Corrupts reconcile/patch on the `TaskRow(task: item)`-in-ForEach path.

**Step 1 —** In `ids.rs`, change the signature and derivation (verified lines 59–68):

```rust
/// Derives the [`NodeId`] for an expression-origin IR node.
///
/// `salt` is the call-site parent id when this expression belongs to an
/// inlined component body (audit C6): without it, two inlinings of the same
/// body derive byte-identical ids and the arena silently drops one.
#[must_use]
pub(crate) fn expr_node_id(expr: &Expr, _kind: ExprNodeKind) -> NodeId {
    compute_node_id(0, ExprTag(EXPR_TAG), expr.span, None)
}
```

becomes:

```rust
/// Derives the [`NodeId`] for an expression-origin IR node.
///
/// `salt` is the call-site parent id when this expression belongs to an
/// inlined component body (audit C6): without it, two inlinings of the same
/// body derive byte-identical ids and the arena silently drops one.
#[must_use]
pub(crate) fn expr_node_id(expr: &Expr, _kind: ExprNodeKind) -> NodeId {
    expr_node_id_salted(expr, _kind, None)
}

/// Salted variant: `salt` (a call-site NodeId) is mixed into the derivation.
#[must_use]
pub(crate) fn expr_node_id_salted(
    expr: &Expr,
    kind: ExprNodeKind,
    salt: Option<NodeId>,
) -> NodeId {
    compute_node_id(
        salt.map(|p| u32::from(p)).unwrap_or(0),
        ExprTag(EXPR_TAG),
        expr.span,
        None,
    )
}
```

   Mechanically: `compute_node_id`'s first parameter is the parent id — confirm its exact signature with `grep -n "pub fn compute_node_id" -A 8 crates/flux-ir/src/arena.rs crates/flux-ir/src/arena/*.rs` and adapt (it may take `parent: NodeId`, in which case pass `salt.unwrap_or(NodeId::from(0u32))`; if `NodeId` lacks `From<u32>`, use the crate's existing conversion idioms). The goal is: **different salt ⇒ different id for the same span**. If `compute_node_id`'s parent parameter cannot produce distinct ids for distinct parents (verify with a two-line unit test: same span, parents 1 and 2, expect different ids), fall back to salting the span: `Span::new(expr.span.file(), expr.span.start() ^ (salt_u32 & 0xFFFF), expr.span.end())` and log which approach was used.

**Step 2 —** In `lower/mod.rs`'s `try_inline_component` (search the name), every call to `expr_node_id` for nodes of the inlined body must become `expr_node_id_salted(expr, kind, Some(call_site_id))`, where `call_site_id` is the NodeId already computed for the inlining call-site view. Grep all `expr_node_id(` call sites in `mod.rs` and salt only those reached from the inline path.

**Step 3 — regression test** in `lower/mod.rs`'s test module:

```rust
#[test]
fn inlined_component_bodies_get_distinct_ids_per_call_site() {
    // Component `Row(text: String) { Text(text: text) }` instantiated twice
    // must produce two distinct NodeIds for the two inner Text nodes.
    let arena = lower_fixture_two_call_sites(); // build: App { Row("a"); Row("b") }
    let text_ids: std::collections::BTreeSet<_> =
        arena.nodes.iter().map(|n| n.id()).collect();
    assert_eq!(text_ids.len(), arena.nodes.len(), "duplicate NodeId in arena");
}
```

**VERIFY**
```bash
cargo test -p flux-ir 2>&1 | tail -3 && cargo test --workspace 2>&1 | grep -cE "test result: ok"
```

**COMMIT:** `fix(C6): salt inlined component NodeIds with the call-site parent id`

---

### T-108 — Lexer: fold negative literals only after a value-capable token [H18] — Danger: MEDIUM

**Files:** `crates/flux-parser/src/lexer.rs`

`-` before a digit always folds into a negative literal, so `x = x-1` silently becomes `x = x` plus a dead `-1`, while `count - 1` (spaces) works. Hidden from every test.

1. Locate the fold: `grep -n "b'-'" crates/flux-parser/src/lexer.rs` (lines ~413 and the number-scanning path ~508-542).
2. The lexer must track the previous significant token. Add a field `prev_can_end_value: bool` to the lexer struct, updated at the end of `next_token`: `true` when the emitted token is `Number | Str | Ident | Rparen | Rbracket | KwTrue | KwFalse`, else `false` (adapt names to the real token enum: `grep -n "pub enum Token" -A 40 crates/flux-parser/src/lexer.rs crates/flux-parser/src/token.rs 2>/dev/null | head -60`).
3. At the minus branch: fold into a negative literal **only when** `!self.prev_can_end_value` (start of file, after an operator, after `(`, `[`, `{`, `,`, `=`, etc.). Otherwise emit `Minus` as its own token.
4. Tests to add in `crates/flux-parser/tests/` (new file `negative_literals.rs`):

```rust
#[test]
fn binary_minus_without_spaces_parses() {
    let ast = flux_parser::parse("fn f() { let y = x-1 }").expect("must parse");
    assert!(matches!(/* walk to the binary expr */, BinOp::Sub), "x-1 must be subtraction");
}
#[test]
fn unary_minus_still_folds() {
    let ast = flux_parser::parse("fn f() { let y = -1 }").expect("must parse");
    assert!(/* contains Int(-1) literal */);
}
```

**VERIFY**
```bash
cargo test -p flux-parser 2>&1 | tail -3 && cargo test --workspace 2>&1 | grep -cE "test result: ok"
```

**COMMIT:** `fix(H18): lexer folds negative literals only when a value cannot be in progress`

---

### T-109 — Generics: one shared variable supply per checker run [H19] — Danger: MEDIUM

**Files:** `crates/flux-types/src/checker/generics.rs`, `crates/flux-types/src/checker.rs`

Per-fn `Supply::default()` resets type-variable ids to 0, colliding across forward-declared functions and with prelude monomorphic bindings (`id(1)` then `id("s")` can spuriously mismatch or corrupt types).

1. Locate: `grep -n "Supply::default()" crates/flux-types/src/checker/generics.rs` (line ~155-183 region).
2. In `checker.rs`, the `Checker` struct must own one `Supply` (add a field if absent: `supply: Supply`). Initialize it in `Checker::new`.
3. In `generics.rs`, replace every `Supply::default()` with borrows/uses of `self.supply` (or a `&mut Supply` parameter threaded from the checker) so ids increase monotonically for the whole check run.
4. Test in `crates/flux-types/tests/generics_supply.rs`:

```rust
#[test]
fn polymorphic_id_reused_across_types_does_not_corrupt() {
    // `fn id[T](x: T) -> T { x }` then `id(1)` then `id("s")` — both calls must type-check.
    let result = flux_types::check_fixture("generic_id_twice");
    assert!(result.is_ok(), "supply collision made id(1);id(\"s\") fail: {result:?}");
}
```

**VERIFY**
```bash
cargo test -p flux-types 2>&1 | tail -3
```

**COMMIT:** `fix(H19): allocate type variables from one checker-wide supply`

---

### T-110 — Stop discarding type errors for component/record args [H20] — Danger: LOW

**Files:** `crates/flux-types/src/checker/apply_callee.rs`

Two `let _ = self.expect(...)` sites (lines ~114 and ~160) discard type errors for component-prop and record-constructor args; positional component args are never checked, so `Button(text: 42)` type-checks.

1. Replace, at each of the two sites (verify exact text first):

```rust
        let _ = self.expect(expected, &arg.ty, arg.span);
```

with:

```rust
        self.expect(expected, &arg.ty, arg.span)?;
```

2. For the positional-args-never-checked gap: in the same file, locate the positional arg loop and ensure each positional arg flows through the same `self.expect(...)?` call (the loop currently applies expectations only to named args; mirror the named-arg branch for positionals, pairing by index against the callee's declared parameter list).

**VERIFY**
```bash
cargo test -p flux-types 2>&1 | tail -3
```
Plus this negative fixture test (add to `crates/flux-types/tests/`):

```rust
#[test]
fn button_with_int_text_is_rejected() {
    let result = flux_types::check_fixture("button_text_int");
    assert!(result.is_err(), "component prop type errors must propagate (H20)");
}
```

**COMMIT:** `fix(H20): propagate type errors for component-prop and record-ctor args`

---

### T-111 — Phase 1 exit gate

```bash
cargo build --workspace && cargo test --workspace 2>&1 | tail -6
```
All green. The conformance vectors added in T-103/T-104/T-105/T-106 must be present under `tests/isa-vectors/`. Log Phase 1 completion in `PROGRESS.md`.

# PHASE 2 — WIRE, DIFFER, DEVSERVER TRUSTWORTHINESS

**Goal:** hot reload cannot corrupt host trees; LAN mode is actually protected; the decoder surface cannot be desynced or DoSed.
**Exit criteria:** differ golden tests pass; fuzz targets exist for socket-facing decoders; token gate test proves rejection.

---

### T-201 — WebSocket token gate: reject must mean reject [C7] — Danger: HIGH

**Files:** `crates/flux-devserver/src/server/session.rs`

Today `serve_client` registers the client into the broadcast list (`shared.register()`) **before** authentication, and `handshook` is set true even when the token was just rejected — a rejected client keeps receiving every `Init`/`Delta` broadcast (full tree + interned strings).

**Step 1 —** Replace (verified at lines 49–60):

```rust
pub(crate) async fn serve_client(stream: TcpStream, shared: Arc<Shared>) -> Result<(), WsError> {
    // Nagle off: patch frames are small and latency-sensitive (spec §3.7).
    stream.set_nodelay(true).map_err(WsError::Io)?;
    // Accept with an explicit config so permessage-deflate is negotiated when the
    // `compression` cargo feature is enabled (see MANIFEST_REQUESTS.md). Clients
    // that never offer the extension (older URLSession/okhttp) still complete the
    // handshake and receive uncompressed frames — enabling compression is
    // backward-compatible and never breaks a connection.
    let socket = accept_async_with_config(stream, Some(websocket_config())).await?;
    let queue = shared.register();
    run_session(socket, queue, shared).await
}
```

with:

```rust
pub(crate) async fn serve_client(stream: TcpStream, shared: Arc<Shared>) -> Result<(), WsError> {
    // Nagle off: patch frames are small and latency-sensitive (spec §3.7).
    stream.set_nodelay(true).map_err(WsError::Io)?;
    // Accept with an explicit config so permessage-deflate is negotiated when the
    // `compression` cargo feature is enabled (see MANIFEST_REQUESTS.md).
    let socket = accept_async_with_config(stream, Some(websocket_config())).await?;
    // Audit C7: the client is registered into the broadcast list ONLY after a
    // successful Hello. A rejected handshake must close the socket, never leave
    // the client subscribed to Init/Delta traffic.
    run_session(socket, shared).await
}
```

**Step 2 —** Restructure `run_session` (currently `async fn run_session(socket: HostSocket, mut queue: UnboundedReceiver<Vec<u8>>, shared: Arc<Shared>)`):

1. Change the signature to `async fn run_session(socket: HostSocket, shared: Arc<Shared>) -> Result<(), WsError>`.
2. Before the main select loop, insert a pre-registration handshake phase:

```rust
    // --- Handshake phase (audit C7) -------------------------------------
    // Read frames until a Hello is seen; `handle_host_frame` routes it to
    // `handle_hello`, which validates the token. Only an ACCEPTED hello
    // registers the client for broadcasts. Anything else — including a
    // rejected token — closes the socket immediately.
    let (writer, mut reader) = socket.split();
    drop(writer); // re-split after registration below
    let mut handshook = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let queue = loop {
        let next = tokio::time::timeout_at(deadline, reader.next()).await;
        match next {
            Err(_) => return Ok(()), // handshake timeout: drop silent clients
            Ok(None) => return Ok(()),
            Ok(Some(Ok(Message::Binary(bytes)))) => {
                for frame in handle_host_frame(&bytes, &shared).await {
                    // replies (Error frame on rejection) — need a writer; see step 3
                }
                if is_hello(&bytes) {
                    if shared.is_registered_client() {
                        handshook = true;
                        break shared.register();
                    }
                    return Ok(()); // rejected: close. No registration, no broadcasts.
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) => return Ok(()),
            Ok(Some(Ok(_))) => {}
        }
    };
```

3. The reply-to-rejection path needs a writer. Adapt mechanically: re-split the socket AFTER the loop (`let (mut writer, reader) = socket.split();` — adjust so the pre-loop reader borrows end before the split) and inside the pre-loop frame loop actually `writer.send(...).await?` for the error frame. If borrow-ordering fights you, restructure instead as: read one frame → if `is_hello`, run `handle_host_frame` with a locally re-split writer, then branch on acceptance. The invariants you must preserve are: (a) zero broadcast frames reach an unregistered client; (b) a rejected client's socket is closed; (c) `handshook` is only ever true for accepted hellos.
4. Add `Shared::is_registered_client(&self) -> bool` (new method): `handle_hello` sets an internal flag / or returns `bool` acceptance — simplest: change `handle_hello` to return `Result<bool, WsError>` (`true` = accepted) and have `handle_host_frame` stash the result in a `Mutex<bool>` on `Shared` named `last_hello_accepted`; `is_registered_client` reads it. Use whichever requires fewer edits; do not change `handle_hello`'s validation logic itself.

**Step 3 — tests.** Add `crates/flux-devserver/tests/token_gate.rs`:

```rust
#[tokio::test]
async fn rejected_client_receives_zero_broadcast_frames() {
    // 1. Start the server loopback with --token "secret" (spawn Shared as tests do).
    // 2. Connect a raw TCP client, complete the WS upgrade, send a Hello with
    //    a WRONG token, read the Error frame reply.
    // 3. Trigger a recompile/broadcast (write a trivial .flux change through
    //    the pipeline helper used by other tests).
    // 4. Assert: reading from the socket yields EOF (closed) within 1s, and
    //    zero binary frames were received after the Error frame.
}
#[tokio::test]
async fn accepted_client_still_receives_init_and_delta() {
    // Same flow with the CORRECT token; assert Init arrives. (Guards against
    // over-tightening the gate.)
}
```

**VERIFY**
```bash
cargo test -p flux-devserver token_gate 2>&1 | tail -4 && cargo test --workspace 2>&1 | grep -cE "test result: ok"
```

**COMMIT:** `fix(C7): register broadcast clients only after token validation; close rejected sockets`

---

### T-202 — Differ: sort multi-inserts into deterministic order [C8] — Danger: MEDIUM

**Files:** `crates/flux-differ/src/diff/algorithm.rs`

`inserted` comes from a hash-set difference, so iteration order is arbitrary — but `Patch::Insert` carries an **absolute index**. Applying `b@1` before `a@0` yields `[a, x, b]` on the host.

Find (verified lines 114–137):

```rust
    let removed: Vec<NodeId> = old_ids.difference(&new_ids).copied().collect();
    let inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
```

Replace with:

```rust
    let removed: Vec<NodeId> = old_ids.difference(&new_ids).copied().collect();
    let mut inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
    // Audit C8: Insert carries an absolute index into the new tree; emitting
    // inserts in hash-set order corrupts the host's child order when more
    // than one row is added per frame. Sort by (parent, index) — stable and
    // deterministic — before any Reattach filtering below.
    inserted.sort_by_key(|id| new_index.get(id).copied());
```

   If `NodeId` or the `(parent, index)` tuple is not `Ord`, sort by their scalar parts: `inserted.sort_by_key(|id| new_index.get(id).map(|(p, i)| (u32::from(*p), u32::from(*i))));` (adapt to the real index-map value type — confirm with `grep -n "new_index" crates/flux-differ/src/diff/algorithm.rs | head`).

Add the golden test to `crates/flux-differ/src/diff/algorithm.rs`'s test module (or a new `#[cfg(test)]` module — restored by T-003):

```rust
#[test]
fn multi_insert_emits_ascending_indices_per_parent() {
    // old tree: parent P with child [x]; new tree: P with [a, b, x].
    let patches = diff_fixture_multi_insert();
    let inserts: Vec<(u32, u32)> = patches.iter().filter_map(|p| match p {
        Patch::Insert { parent, index, .. } => Some((u32::from(*parent), u32::from(*index))),
        _ => None,
    }).collect();
    let mut sorted = inserts.clone();
    sorted.sort();
    assert_eq!(inserts, sorted, "inserts must be emitted in (parent, index) order");
}
```

**VERIFY**
```bash
cargo test -p flux-differ 2>&1 | tail -3
```

**COMMIT:** `fix(C8): differ emits multi-insert patches in deterministic (parent, index) order`

---

### T-203 — Differ: new top-level components must be inserted, not dropped [C9] — Danger: MEDIUM

**Files:** `crates/flux-differ/src/diff/algorithm.rs`

`new_index.get(id)` is `None` for arena roots (only nodes with an arena parent are indexed), so the differ never emits `Insert` for a new root, while removal IS emitted — asymmetric. Adding a top-level component during hot reload never reaches the host.

In the insert loop (verified lines 129–136), find:

```rust
        if let Some((parent, index)) = new_index.get(id).copied() {
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent,
                index,
                node: to_ref(&n),
            });
        }
```

Replace with:

```rust
        if let Some((parent, index)) = new_index.get(id).copied() {
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent,
                index,
                node: to_ref(&n),
            });
        } else if is_root_of_new(new, id) {
            // Audit C9: arena roots have no entry in `new_index` (no parent),
            // so new top-level components were silently dropped on hot reload.
            // Emit the insert against the stable synthetic wrapper id the
            // devserver's tree.rs uses for the multi-root Init (§D.12.2).
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent: synthetic_root_id(),
                index: root_position(new, id),
                node: to_ref(&n),
            });
        }
```

   Add the two helpers at the bottom of the same file (adapt to real APIs):
   - `fn is_root_of_new(new: &Tree, id: &NodeId) -> bool` — true when no node in `new` lists `id` as a child. Reuse the existing `root_ids`-style helper if one exists in this crate (`grep -n "fn root_ids\|fn roots" crates/flux-differ/src -r`).
   - `fn synthetic_root_id() -> NodeId` — the same id `crates/flux-devserver/src/pipeline/tree.rs` computes for the multi-root wrapper (verified there at line 63: `flux_ir::compute_node_id(0, NodeKind::Component, Span::new(0, 0, 0), None)`). Either import that helper or duplicate the exact expression; add a comment binding the two.
   - `fn root_position(new: &Tree, id: &NodeId) -> u32` — the index of `id` among the new tree's roots.

Add the test:

```rust
#[test]
fn new_root_component_is_inserted_on_hot_reload() {
    // old: one root component A; new: components A and B (B is new).
    let patches = diff_fixture_new_root();
    assert!(patches.iter().any(|p| matches!(p, Patch::Insert { .. })),
        "a newly added top-level component must produce an Insert patch");
}
```

**VERIFY**
```bash
cargo test -p flux-differ 2>&1 | tail -3
```

**COMMIT:** `fix(C9): differ inserts new top-level components against the synthetic root`

---

### T-204 — Checked casts for all wire length prefixes [H14] — Danger: MEDIUM

**Files:** `crates/flux-ir-serde/src/frame.rs`, `crates/flux-ir-serde/src/emit.rs`, `crates/flux-ir-serde/src/wire/string_entry.rs`, `crates/flux-ir-serde/src/wire/child.rs`, `crates/flux-ir-serde/src/telemetry.rs`

Pervasive `len() as u16` silently truncates for values > 64 KiB, and hosts then slice the wrong blob range.

1. Add one helper per crate (in `flux-ir-serde`, put it in `lib.rs`):

```rust
/// Checked length-prefix conversion. A silent `as u16` truncation desyncs
/// every host decoder (audit H14): fail the encode instead.
pub(crate) fn u16_len(n: usize, what: &'static str) -> Result<u16, EncodeError> {
    u16::try_from(n).map_err(|_| {
        EncodeError::new(format!("{what} length {n} exceeds u16 prefix width"))
    })
}
```

   Adapt to the crate's real error type (`grep -n "enum EncodeError\|enum SerdeError" crates/flux-ir-serde/src | head`); if the emit functions do not return `Result`, convert the minimal enclosing set to do so, driven by cargo errors — if that snowballs beyond `flux-ir-serde`, log `BLOCKED: error plumbing too wide` and instead make the helper panic with an explicit message (loud, still not silent):

```rust
pub(crate) fn u16_len(n: usize, what: &'static str) -> u16 {
    u16::try_from(n).unwrap_or_else(|| panic!("{what} length {n} exceeds u16 prefix width (audit H14)"))
}
```

2. Sweep the five files: `grep -n "as u16" crates/flux-ir-serde/src/frame.rs crates/flux-ir-serde/src/emit.rs crates/flux-ir-serde/src/telemetry.rs crates/flux-ir-serde/src/wire/string_entry.rs crates/flux-ir-serde/src/wire/child.rs`. For every occurrence whose operand is a `len()`/count (NOT an already-bounded enum discriminant or a bitfield), replace `x as u16` with `u16_len(x, "<what>")` (or the panicking variant). Add a comment at each site only if the `what` label is not self-evident.
3. Same sweep for `crates/flux-ir/src/arena/blob.rs` (P2.11, folded here because it is the same defect class): lines ~48, 76, 82, 137, 146, 183. Use the crate-local equivalent helper; if `blob.rs` is infallible code, use the panicking variant.

**VERIFY**
```bash
cargo test -p flux-ir-serde 2>&1 | tail -3
cargo test -p flux-ir 2>&1 | tail -3
grep -rn "len() as u16" crates/flux-ir-serde/src crates/flux-ir/src/arena/blob.rs || echo "NO-UNSAFE-CASTS"
```
Expected: `NO-UNSAFE-CASTS` (or only hits on enum discriminants you explicitly left, each with a `// bounded` comment).

**COMMIT:** `fix(H14): checked u16 length prefixes across serde emit paths and arena blobs`

---

### T-205 — Bounded broadcast + slow-client eviction + handshake timeout [H15] — Danger: MEDIUM

**Files:** `crates/flux-devserver/src/server.rs`, `crates/flux-devserver/src/debug_bridge.rs`, `crates/flux-devserver/src/server/session.rs`

Unbounded broadcast channels with no timeout for non-handshaked clients = unauthenticated remote memory-DoS (compounds C7).

1. Locate channel construction: `grep -rn "unbounded_channel\|broadcast::channel" crates/flux-devserver/src | head`. Replace every broadcast channel creation with a bounded one: `tokio::sync::broadcast::channel(1024)`.
2. In `session.rs`, the receiver loop handles `broadcast::error::RecvError`: add a match arm — on `Lagged(n)`, log at `warn!` and **close the session** (`return Ok(())`). A client that cannot keep up must not silently consume unbounded memory; dropping it is the devserver's documented policy (add the one-line comment).
3. The handshake timeout was already added in T-201 step 2 (10 s `timeout_at`). If T-201 was BLOCKED, add the same `tokio::time::timeout` wrapper around the pre-handshake read loop here.
4. In `debug_bridge.rs`, apply steps 1–2 to its broadcast/queue channels as well.

**VERIFY**
```bash
grep -rn "unbounded_channel" crates/flux-devserver/src || echo "ALL-BOUNDED"
cargo test -p flux-devserver 2>&1 | tail -3
```

**COMMIT:** `fix(H15): bounded broadcast channels; lagging clients evicted; handshake timeout`

---

### T-206 — Broadcast must not race the pipeline lock [H16] — Danger: MEDIUM

**Files:** `crates/flux-devserver/src/watch.rs`

Compile runs under the pipeline lock, but broadcast happens after the guard drops; a Hello served in between gets `Init(new)` then `Delta(old→new)` and corrupts its tree.

1. Locate the sequence: `grep -n "pipeline.lock\|broadcast\|send(" crates/flux-devserver/src/watch.rs | head -20`.
2. Restructure so the broadcast send happens while the `pipeline.lock()` guard is still held (a `tokio::sync::broadcast::Sender::send` is non-blocking and safe to call under a std Mutex). Move the `send(...)` calls inside the lock scope. If the broadcast payload must be computed outside (borrow rules), compute it inside, drop the guard, and send immediately — but then ALSO add this guard at the receiving end: in `session.rs`, when a client has just completed its Hello, record `last_init_seq` from the Init frame's sequence field, and skip any Delta whose `seq <= last_init_seq`. Implement whichever half is smaller; prefer broadcast-under-lock.
3. Test (`crates/flux-devserver/tests/watch_race.rs`): start a server, connect + handshake a client, then trigger 5 rapid file edits; assert the client never receives a Delta whose parent-epoch precedes its Init's epoch (adapt to how frames expose epochs; if frames carry no epoch, assert instead that the client's applied-tree checksum after the burst equals the final disk state via the existing tree-checksum test helper).

**VERIFY**
```bash
cargo test -p flux-devserver 2>&1 | tail -3
```

**COMMIT:** `fix(H16): broadcast Init/Delta without a Hello-race window on the pipeline lock`

---

### T-207 — AsyncBridge: per-session lifecycle and real resume wiring [H17] — Danger: HIGH ⚠ STOP-AND-ASK if capability-completion plumbing is unclear

**Files:** `crates/flux-devserver/src/async_bridge.rs`, `crates/flux-devserver/src/server/session.rs`

The server-wide bridge is never pruned on disconnect; `settle()` has zero non-test callers, so parked handlers never resume in production; a stale `early` value can even resume a future session's handler.

1. `grep -n "struct AsyncBridge\|static\|lazy_static\|OnceLock" crates/flux-devserver/src/async_bridge.rs | head` — if the bridge is a process-wide singleton, change it to per-session: construct it in `serve_client` (or wherever a session's pipeline handle is created), store it on the session's `Shared`/state struct, and drop it when the session ends (Rust ownership does this automatically once the singleton is gone).
2. Delete or gate `settle()`: `grep -rn "settle(" crates/flux-devserver/src | grep -v test`. If all callers are tests, mark it `#[cfg(test)]` and add a production resume path instead: capability completion (the CALL_CAP async result arriving from the host, or the dev-server-side HTTP stub finishing) must call `bridge.resume(handler_id, value)`. Locate where async capability results currently land (`grep -rn "AsyncResolver\|resume\|AWAIT" crates/flux-devserver/src | head -30`) and wire the completion callback. If the devserver has no real async capability source yet, it is acceptable to implement only (1) + clearing, and log `T-207 PARTIAL: resume wiring deferred (no production async source)` — but the stale-`early` leak MUST still be fixed: clear `early` values on disconnect.
3. Acceptance test `crates/flux-devserver/tests/bridge_lifecycle.rs`: two sequential simulated sessions; the first parks a handler (AWAIT) with a value injected via `early`, disconnects WITHOUT resuming; the second session must not observe any resume from the first session's leftover state.

**VERIFY**
```bash
cargo test -p flux-devserver 2>&1 | tail -3
```

**COMMIT:** `fix(H17): per-session AsyncBridge cleared on disconnect; no cross-session resumes`

---

### T-208 — Devserver: remove blocking lock calls from the async reactor [P2.1] — Danger: LOW

**Files:** `crates/flux-devserver/src/server/session.rs`

`pipeline.lock()` (std blocking Mutex) is taken inline on the async reactor at lines ~362 and ~381; a compile stalls all tokio workers. The codebase already has a `blocking()` helper (used at line 218).

At each of the two sites, wrap the lock acquisition in the existing helper, mirroring the pattern at line 225–226:

```rust
    let shared_req = Arc::clone(shared);
    let result = blocking(move || shared_req.pipeline.lock().<the existing method chain>).await;
```

**VERIFY:** `cargo build -p flux-devserver 2>&1 | tail -3` — then `grep -n "pipeline.lock()" crates/flux-devserver/src/server/session.rs` and confirm every remaining inline use is inside a `blocking(move || ...)` closure.

**COMMIT:** `fix(P2.1): route remaining inline pipeline.lock() calls through the blocking helper`

---

### T-209 — Deleting a .flux file must remove its components [P2.2] — Danger: MEDIUM

**Files:** `crates/flux-devserver/src/watch.rs`, `crates/flux-devserver/src/pipeline.rs` (read-only usage; method lives where `Pipeline` is defined)

Deleting a `.flux` file only logs; `Pipeline` has no removal API, so deleted components keep rendering forever.

1. `grep -n "ENOENT\|remove\|RemoveFile\|event.kind" crates/flux-devserver/src/watch.rs | head` — locate the event branch handling deletion (~lines 119–130).
2. Add `Pipeline::remove_file(&mut self, file_id: FileId)` (in whichever module defines `Pipeline`; search `impl Pipeline`): drop the file's interned strings, handlers, IR arena entries, and component registrations exactly as `set_file`/compile registers them (mirror the data structures it writes; each gets a removal). Reuse any existing per-FileId indexing.
3. In the watch branch, on ENOENT/`Remove` events, call it, then run the normal re-Init broadcast path.
4. Test: fixture project with two components; delete one file; the next Init must contain only the surviving component's nodes (use the same pipeline-test helper other devserver tests use).

**VERIFY:** `cargo test -p flux-devserver 2>&1 | tail -3`

**COMMIT:** `fix(P2.2): deleted .flux files drop their components from the pipeline`

---

### T-210 — DevTools endpoint: config-driven bind, token, per-connection upgrade [P2.3] — Danger: MEDIUM

**Files:** `crates/flux-devserver/src/server.rs`, `crates/flux-devserver/src/debug_bridge.rs`

The DevTools endpoint is hard-bound to `0.0.0.0:7333` ignoring config, has no token, telemetry carries request URLs/body snippets, and the inline WS upgrade lets one silent client block all DevTools connections.

1. Bind: find `7333` (`grep -rn "7333" crates/flux-devserver/src`); replace the hard-coded address with one read from the server config struct (the same struct that carries `--ws-host`/`--token`; add `devtools_host: Option<String>`/`devtools_port: u16` defaults `127.0.0.1`/`7333` if the struct lacks them — config struct lives in `crates/flux-devserver/src/config.rs` or `server.rs`; grep `pub struct.*Config`).
2. Token: reuse the Hello/pairing mechanism from the main WS path minimally — accept an optional `?token=` query param on the DevTools upgrade and reject mismatches with HTTP 401 before upgrading (no new protocol).
3. Upgrade: `grep -n "accept_hdr\|upgrade" crates/flux-devserver/src/debug_bridge.rs` — move the WS upgrade into `tokio::spawn` per connection so a silent client cannot block the accept loop.
4. Telemetry scrubbing: `grep -rn "url\|body" crates/flux-devserver/src/debug_bridge.rs | head` — remove any request URL/body capture from emitted telemetry (keep counts and status codes).

**VERIFY:** `cargo test -p flux-devserver 2>&1 | tail -3` plus `grep -rn "0.0.0.0:7333\|127.0.0.1:7333" crates/flux-devserver/src` shows no hard-coded bind remains.

**COMMIT:** `fix(P2.3): DevTools endpoint config-driven bind, token-gated, per-connection upgrade, scrubbed telemetry`

---

### T-211 — Static asset serving must canonicalize and reject escapes [P2.4] — Danger: MEDIUM

**Files:** `crates/flux-devserver/src/assets.rs`

The traversal guard rejects `..`/absolute paths but never canonicalizes and follows symlinks; a symlink inside the project root serves any file on disk over unauthenticated :7332.

Find the guard (~lines 111–120, `grep -n "canonicalize\|\.\." crates/flux-devserver/src/assets.rs`). After the existing rejection checks, add:

```rust
    // Audit P2.4: canonicalize and re-verify the prefix AFTER resolution so
    // symlinks inside the project root cannot escape it.
    let canonical_root = root.canonicalize().map_err(|_| AssetError::NotFound)?;
    let canonical_path = path.canonicalize().map_err(|_| AssetError::NotFound)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(AssetError::NotFound);
    }
```

(Adapt error names to the file's real ones.) Add a test creating a symlinked file inside the root pointing outside, asserting 404/NotFound.

**VERIFY:** `cargo test -p flux-devserver assets 2>&1 | tail -3`

**COMMIT:** `fix(P2.4): canonicalize asset paths and re-verify root containment (symlink escape)`

---

### T-212 — Differ must patch handler additions/removals on stable nodes [P2.5] — Danger: MEDIUM

**Files:** `crates/flux-differ/src/diff/emit.rs`

Handler ids present in only one of old/new are skipped, and handlers are not part of the content id — changing a node's handler set ships nothing on hot reload.

In `emit.rs` (~lines 21–35), the patch-emission loop must, for every matched (unchanged-content) node pair, diff the two handler-id sets and emit `Patch::SetHandler` (confirm the patch variant name: `grep -n "enum Patch" -A 30 crates/flux-ir-serde/src/wire/patch.rs` — use whatever variant exists for attaching a handler; if none exists, log `BLOCKED: no SetHandler patch variant` and add a `SIDECAR` row instead of inventing wire format).

Add to the differ tests (module restored in T-003):

```rust
#[test]
fn handler_added_to_stable_node_produces_patch() {
    // old: Button with no handler; new: same Button with onTap. Must emit
    // exactly one handler patch, no Remove/Insert.
}
```

**VERIFY:** `cargo test -p flux-differ 2>&1 | tail -3`

**COMMIT:** `fix(P2.5): emit handler patches for added/removed handler ids on stable nodes`

---

### T-213 — Handler equality must include captured signals [P2.6] — Danger: LOW

**Files:** `crates/flux-differ/src/diff/compare.rs`

`handlers_equal` (~line 107) ignores `captured_signals`, which `hash_closure` treats as behavior-changing → stale signal wiring after hot reload.

Find `fn handlers_equal` and extend it: after the existing comparisons, add

```rust
        if a.captured_signals != b.captured_signals {
            return false;
        }
```

(adapt field names: `grep -n "captured_signals" crates/flux-differ/src crates/flux-ir-serde/src/wire/closure_ref.rs | head`).

**VERIFY:** `cargo test -p flux-differ 2>&1 | tail -3` — plus one new test: same bytecode, different captured set ⇒ not equal.

**COMMIT:** `fix(P2.6): handlers_equal compares captured_signals`

---

### T-214 — Component-id fold must not collide on multiples of 0x100 [P2.7] — Danger: MEDIUM

**Files:** `crates/flux-syntax/src/ids/node_id.rs`

`component_id` is folded into 7 bits (`^ (mul(0x9E) as u8)`, lines ~79–83): ids differing by multiples of 0x100 collide, so structurally identical nodes of *different components* can share content ids.

Replace the 7-bit fold with a 4-raw-byte fold into the hash: locate the fold, and change the mixing of `component_id` from one byte to all four LE bytes (e.g. extend the existing mixer with `hash ^= component_u32; hash = hash.wrapping_mul(0x0100_0193);`). Preserve the function's public signature. Add a unit test: two component ids 0x0100 apart must produce different node ids for identical spans.

**VERIFY:** `cargo test -p flux-syntax 2>&1 | tail -3` and `cargo test --workspace 2>&1 | grep -cE "test result: ok"` (golden vectors elsewhere may embed old ids — if exactly the golden fixtures fail, regenerate them per the golden-file instructions in those tests and log it).

**COMMIT:** `fix(P2.7): fold component_id into node ids as 4 raw bytes`

---

### T-215 — Opcode::ALL must include BoolEq [P2.8] — Danger: LOW

**Files:** `crates/flux-syntax/src/opcode.rs`

`Opcode::ALL` (~lines 175–241) omits `BoolEq` though the enum/raw/decode/width tables have it — the same drift class FLUX-078 fixed for list ops.

Find the `ALL` array and add `Opcode::BoolEq` in raw-value order (between the Eq-family entries; check `grep -n "BoolEq" crates/flux-syntax/src/opcode.rs` for its raw byte and neighbors). Add a test asserting `Opcode::ALL` length equals the number of enum variants and that every variant's `raw()/decode()/operand_width()` round-trips:

```rust
#[test]
fn all_covers_every_opcode_and_round_trips() {
    for op in Opcode::ALL {
        assert_eq!(Opcode::from_raw(op.raw()), Some(op));
    }
}
```

**VERIFY:** `cargo test -p flux-syntax 2>&1 | tail -3`

**COMMIT:** `fix(P2.8): Opcode::ALL includes BoolEq; ALL/round-trip coverage test`

---

### T-216 — Detect prop-index collisions at compile time [P2.9] — Danger: MEDIUM

**Files:** `crates/flux-ir/src/lower/mod.rs`

`prop_index_for_name` truncates FNV-32 to u16 (lines ~972–982); with ~300 distinct prop names the birthday bound is ~50% — two props sharing a slot silently overwrite each other's `SET_FIELD`.

Keep the u16 (it is wire-visible); make collisions loud. In `lower/mod.rs`, where the lowering context is created, add a `prop_indices: std::collections::HashMap<u16, String>` registry. Every `prop_index_for_name` call site in lowering registers the name: if the index is already registered to a DIFFERENT name, return a lowering error: `"prop name collision on FNV slot {idx}: '{a}' vs '{b}' — extend PropIdx or rename (audit P2.9)"`. Mechanically: wrap the free function in `fn intern_prop_index(&mut self, name: &str) -> Result<PropIdx, LowerError>` used by lowering internals, while the free function stays public for tests. Add a unit test with two colliding names (find a real collision by brute force over generated names, or construct the map directly).

**VERIFY:** `cargo test -p flux-ir 2>&1 | tail -3`

**COMMIT:** `fix(P2.9): lowering errors on FNV prop-index collisions instead of overwriting`

---

### T-217–T-221 — Compiler-correctness sweep (smaller, batched) — Danger: MEDIUM each

Execute in order; each is one focused fix. VERIFY for each: `cargo test -p <touched-crate> 2>&1 | tail -3`, full suite at the end.

| ID | Finding | File(s) | Instruction |
|---|---|---|---|
| T-217 | P2.10 mono cursor desync | `crates/flux-ir/src/lower/mono.rs` (~95–107), `crates/flux-types/src/checker/infer.rs` (~33–35) | The instantiation cursor is consumed trailing-block-first in one place and args-first in the other. Find both cursors (`grep -n "cursor\|instantiation" mono.rs infer.rs | head`), pick the checker's order as canonical, and make mono.rs consume in the SAME order. Add a test instantiating `Option[T]` with two type args asserting the right specialization maps to the right call site. |
| T-218 | P2.11 arena blob truncations | `crates/flux-ir/src/arena/blob.rs` (~48, 76, 82, 137, 146, 183) | Folded into T-204 step 3. If you executed T-204, mark this DONE referencing that commit; else perform the same checked-cast sweep here now. |
| T-219 | P2.12 content-address recursion + root collapse | `crates/flux-ir/src/arena/content_address.rs` (~37–74, 128–204) | (a) Convert the recursive hash to an explicit stack loop (same visit order!) so deep trees cannot overflow. (b) Roots all get parent=0/position=0, so identical roots or duplicate ForEach keys collapse to one id: mix the root's index among the arena's root list into the hash (thread a `root_slot: usize` through the entry point only). Test: two identical sibling components at distinct top-level positions get distinct content ids. |
| T-220 | P2.13 ForEach key deps + checker | `crates/flux-ir/src/lower/mod.rs` (~568–578), `crates/flux-types/src/checker/infer.rs` (~183) | (a) In ForEach lowering, include the key expression's signal reads in `signal_deps` (collect them the same way body deps are collected — mirror the existing `collect` call for the body on the key expr). (b) In infer.rs, verify `key:` is a function of the element: check its type's parameter list unifies with the element type; error `"ForEach key must be a function of the element"` otherwise. Tests for both. |
| T-221 | P2.14 double `await` double-deposit | `crates/flux-ir/src/lower/bytecode.rs` (~1235–1247) | In the `ExprKind::Await` arm (verified at those lines), after emitting AWAIT, `MOV` the result out of r0 into a fresh register and return THAT register: `let out = self.alloc_reg()?; self.code.push(raw::MOV); self.code.push(out); self.code.push(0u8); Ok(out)`. Test: handler computing `await f() + await g()` with f→1, g→2 yields 3, not 4. |

**VERIFY (after T-221):** `cargo test --workspace 2>&1 | tail -6`

**COMMIT (one per task):** messages of the form `fix(P2.x): <one-liner>`.

---

### T-222 — Phase 2 exit gate

- `cargo test --workspace` green.
- `ls tests/isa-vectors/` contains `typed_arith.json`.
- `ls fuzz/fuzz_targets/` reviewed: at minimum `decode_frame` exists; record remaining unfuzzed decoders (`TelemetryFrame`, `DebugCommandFrame`, `AwaitSuspend/Resume`, `DispatchReport`, `HostAnnounce`, intern-string, `decode_value_blob`, `validate_bytecode`) in `PROGRESS.md` as `SIDECAR` rows (fuzz targets for them are Phase 6 hygiene, T-606).

Log Phase 2 completion, proceed to Phase 3.

# PHASE 3 — HOST RUNTIMES: SWIFT iOS, KOTLIN ANDROID, CROSS-PLATFORM PARITY

**Goal:** capabilities execute on both platforms; no leaks or main-thread freezes; ForEach identity is one shared scheme; the drift matrix (audit §4) is closed.
**Swift paths:** `runtimes/ios/FluxHost/Sources/FluxHost/` · **Kotlin paths:** `runtimes/android/host/src/main/kotlin/dev/flux/host/`

## 3A. Swift iOS host

---

### T-301 — Permission table: add Http (14) and Persist (15) [C10 / D4] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/Permission.swift`

`requiredPermission` has no cases for cap 14/15 and falls to `default: nil`; the VM gate treats `nil` as unknown ⇒ unconditional `CAPABILITY_DENIED`. Http and Persist can never execute on iOS.

1. Check the `PermissionKind` enum has the needed cases: `grep -n "enum PermissionKind" -A 15 runtimes/ios/FluxHost/Sources/FluxHost/Permission.swift`. If `.network` / `.storage` are absent, add them to the enum (mirroring `crates/flux-types/src/capabilities/permission.rs`, which maps `(14,_)→Network`, `(15,_)→Storage`).
2. In `requiredPermission`, find (verified at lines 79–81):

```swift
    case 13: .nativeModule // NativeModule.invoke — explicit .native grant (FLUX-046)
    default: nil
    }
```

Replace with:

```swift
    case 13: .nativeModule // NativeModule.invoke — explicit .native grant (FLUX-046)
    case 14: .network      // Http.get/post/... — audit C10/D4: missing case hard-denied all Http
    case 15: .storage      // Persist.get/set — audit C10/D4: missing case hard-denied all Persist
    default: nil
    }
```

**VERIFY**
```bash
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | tail -5
```
Note which capability tests were red before (C10 step 2 makes one structurally red — T-302 fixes that half).

**COMMIT:** `fix(C10): requiredPermission covers caps 14 (network) and 15 (storage)`

---

### T-302 — Http/Persist must share ONE request store between registry and resolver [C10] — Danger: MEDIUM

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift`, `runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift`

`Registry.swift` mints `httpPersistEntries(store: HttpRequestStore(), transport: URLSessionHttpTransport())` inside `.dev` (~lines 247–250, verified), while `FluxExecutor.swift` (~line 165, verified) builds `HttpAsyncResolver` with its **own** `httpRequests` store — the resolver never finds the pending request, so cells settle wrong.

1. In `FluxExecutor.swift`, find where the capability registry is constructed/assigned (search `capRegistry` and `CapabilityRegistry`). Replace the hardcoded `.dev` registry construction with one built from the executor's own store/transport:

```swift
        // Audit C10: the registry's Http/Persist entries MUST share the same
        // HttpRequestStore the async resolver polls, otherwise a request
        // written by the capability is never observed by the resolver.
        let registry = CapabilityRegistry(
            entries: Self.mintDevEntries(
                store: httpRequests,
                transport: httpTransport,
                nativeHost: nativeHost
            )
        )
```

   Mechanically: adapt to the real constructor signature (`grep -n "init(" runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift | head`); the goal is that the SAME `httpRequests` and `httpTransport` values flow into `httpPersistEntries`. If `httpPersistEntries` is private to `Registry`, expose it as `static` and call it from the executor, or move the registry construction into an `Registry.init(store:transport:nativeHost:)` convenience — either is fine, do not duplicate the store.
2. In `Registry.swift`, the `.dev`-path code that calls `httpPersistEntries(store: HttpRequestStore(), transport: URLSessionHttpTransport())` must be removed or rerouted through the injected store/transport parameters (keep `HttpRequestStore()` only as the default argument of the convenience init when callers pass nothing, i.e. tests).
3. Fix the structurally-red test: `grep -rn "testHttpGetJsonResolvesToRecordViaResolver" runtimes/ios/FluxHost/Tests/` — it asserts `.record` on what is actually `.int`. After (1)+(2), the resolver finds the pending request and the assertion should pass AS-IS. If it still fails, run the test with logging and ensure the flow goes through `CALL_CAP` (the test must NOT call `registry.lookup(...)!(...)` directly — rewrite it to dispatch a CALL_CAP through the VM/execitor so the permission gate added in T-301 is exercised).

**VERIFY**
```bash
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | grep -E "Executed|failed" | tail -5
```

**COMMIT:** `fix(C10): single HttpRequestStore shared by capability registry and async resolver`

---

### T-303 — Swift list-op operand widths 4/3 → 3/2 [C11 / D3] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/OpCodes.swift`

The execution positions in `FluxBytecodeVM.swift` (both dispatch paths, verified lines 456–479 and 1002–1025) already read `u8(0)/u8(1)/u8(2)` correctly — but the operand **length** table says `.listInsert: 4` / `.listRemove: 3`, so the program counter skips one byte after every list op and execution misaligns. The compiler emits 3/2 (`bytecode.rs:613-624`); Kotlin is already correct (`vm/Opcode.kt:76-77`).

Find (verified lines 187–188):

```swift
        case .listInsert: 4
        case .listRemove: 3
```

Replace with:

```swift
        case .listInsert: 3 // LIST_INSERT list(u8), idx(u8), val(u8) — audit C11/D3
        case .listRemove: 2 // LIST_REMOVE list(u8), idx(u8)
```

Then add ISA vectors (iOS currently has none for these ops): create `tests/isa-vectors/list_ops.json` (repo root) with rows `{op, raw_operands_hex, expected}` for insert/remove/clear/removeItem, and a Swift test `ListOpIsaVectorsTests` in `FluxHostTests` that decodes each row, executes it, and compares the resulting list register. Mirror the Kotlin test in `FluxBytecodeVmTest.kt:267-285` for the expected semantics.

**VERIFY**
```bash
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | tail -5
```

**COMMIT:** `fix(C11): Swift list-op operand widths match the 3/2 compiler encoding; ISA vectors added`

---

### T-304 — Rust oracle: fix list-op register positions (u8(2)/u8(3) → u8(1)/u8(2)) [C11] — Danger: LOW

**Files:** `crates/flux-vm-ref/src/vm.rs`

The oracle reads operands beyond the 3/2-byte window ⇒ `operands[3]` index-out-of-bounds panic on any compiled list insert/remove — the behavioral ground truth cannot execute its own compiler's bytecode for these ops.

Find (verified lines 649–667):

```rust
            Opcode::ListInsert => {
                let idx = usize::from(instr.u8(2));
                let val = reg!(instr.u8(3));
                let list = instr.u8(1);
```

Replace with:

```rust
            Opcode::ListInsert => {
                // LIST_INSERT list(u8), idx(u8), val(u8) — audit C11: the
                // oracle read positions 2/3/1, one past the 3-byte operand
                // window, panicking on any compiled insert.
                let idx = usize::from(instr.u8(1));
                let val = reg!(instr.u8(2));
                let list = instr.u8(0);
```

Find:

```rust
            Opcode::ListRemove => {
                let idx = usize::from(instr.u8(2));
                let list = instr.u8(1);
```

Replace with:

```rust
            Opcode::ListRemove => {
                // LIST_REMOVE list(u8), idx(u8) — audit C11 (same off-by-one).
                let idx = usize::from(instr.u8(1));
                let list = instr.u8(0);
```

Then verify the oracle can execute compiler output end-to-end: add `crates/flux-vm-ref/tests/list_ops_from_compiler.rs` that lowers a tiny program containing `list.insert(0, x)` and `list.remove(0)` through flux-ir and executes it — asserting no panic and the correct final list.

**VERIFY**
```bash
cargo test -p flux-vm-ref 2>&1 | tail -3
```

**COMMIT:** `fix(C11): oracle list-op operand positions match the 3/2 compiler encoding`

---

### T-305 — Swift f64→i64 conversion must saturate like the oracle [H1] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift`

`Int64(v)` traps (crash) for NaN/±inf/out-of-range in the three conversion sites (~lines 301–304, 869–872, 1259–1262). The Rust oracle saturates (`vm.rs:546`).

1. Add a helper near the other numeric helpers in the same file:

```swift
    /// f64 → i64 with oracle-parity saturation (audit H1): NaN → 0, values
    /// outside Int64 range clamp to min/max. `Int64(v)` would trap.
    static func f64ToI64(_ v: Double) -> Int64 {
        if v.isNaN { return 0 }
        if v >= 9.223372036854776e18 { return Int64.max }
        if v <= -9.223372036854776e18 { return Int64.min }
        return Int64(v)
    }
```

2. Replace the three `Int64(...)` conversion sites with `Self.f64ToI64(...)` (or the file's local convention). Locate them: `grep -n "Int64(" runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift | grep -v "Int64.max\|Int64.min\|UInt64"` and convert only the `.f64ToI64`/conversion-opcode sites (the three ranges above).
3. Test in `FluxHostTests`: NaN→0, 1e30→Int64.max, -1e30→Int64.min, 3.7→3 (matching oracle `vm.rs:546` semantics — read it to confirm rounding is truncation).

**VERIFY:** Swift host test suite green (command as T-303).

**COMMIT:** `fix(H1): f64ToI64 saturates instead of trapping, matching the Rust oracle`

---

### T-306 — Remove the resumable-interpreter's phantom null pre-population [H2] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift`

The pre-populates ALLOC_RECORD with positional nulls (~lines 945–948), which `run` and the oracle do not — after any AWAIT, records carry phantom null fields.

Locate the loop in `execTailWith` (search `grep -n "ALLOC_RECORD\|allocRecord" runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift | head`) and delete the pre-population loop (the block writing positional `.null` entries into a freshly allocated record inside the resume path only). Comment to add where it was:

```swift
        // Audit H2: the resumable path pre-populated records with positional
        // nulls, diverging from `run` and the oracle; after any AWAIT every
        // record carried phantom null fields. Allocation now starts empty on
        // all three interpreters.
```

Test: a handler that awaits then allocates a record and reads a field set before the await must not observe a phantom extra field (port the corresponding oracle behavior).

**VERIFY:** Swift host suite green; specifically any AwaitResume tests still pass — if a test DEPENDED on the nulls, it was encoding the bug: fix the test's expectation to match the oracle, and log it.

**COMMIT:** `fix(H2): resumable interpreter no longer pre-populates records with phantom nulls`

---

### T-307 — Reconciler must not wipe the whole view tree every frame [H3] — Danger: HIGH

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift`

`built.removeAll()` (~line 241) runs on **every** Delta frame: all native view identity is destroyed per frame (scroll/focus/text state lost), `adapter.destroy`/`onCleanup` never run, and same-frame remove/reorder operate on freshly rebuilt views.

Find the `removeAll()` call (verified at line 241). Replace the unconditional wipe with a condition: only wipe when the current frame's patches contain a Replace targeting the root node. Mechanically:

1. Before the wipe, inspect the frame's patch list (the reconciler has the patches; confirm the variable: `grep -n "patches\|Patch" runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift | head`).
2. Replace:

```swift
        built.removeAll()
```

with:

```swift
        // Audit H3: wiping `built` on every frame destroyed all native view
        // identity (scroll/focus/text loss) and skipped adapter.destroy.
        // Only a Replace of the ROOT invalidates the whole map.
        let rootReplaced = patches.contains {
            if case .replace(let id, _, _) = $0 { return id == rootId }
            return false
        }
        if rootReplaced { built.removeAll() }
```

   Adapt to the real patch enum spelling (`grep -n "case replace\|enum Patch" runtimes/ios/FluxHost/Sources/FluxHost/*.swift | head`) and the real root-id variable name (the id the Init frame seeded). If the reconciler processes patches one at a time rather than holding the list, compute `rootReplaced` before the loop from the frame's patch array.
3. Test: apply an Init, then a Delta patching one Text prop; assert the same UIKit view instance is retained (identity check) and only its prop changed. Apply a root Replace; assert the map was rebuilt.

**VERIFY:** Swift host suite green.

**COMMIT:** `fix(H3): reconciler wipes view identity only on root Replace, not every frame`

---

### T-308 — Reconciler leak sweep: destroy unreachable subtrees and prune stale rows [H4] — Danger: HIGH

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift`

Subtree removal destroys only the root (descendants stay in `built` forever); full frames never destroy absent ids; ForEach expansion never prunes stale rows → unbounded memory growth + zombie UIControl targets.

1. After the reconcile pass completes (end of the per-frame apply function), add a reachability sweep:

```swift
        // Audit H4: anything still in `built` that is not reachable from the
        // current tree is stale — destroy its native view and drop it.
        var reachable = Set<UInt32>()
        func mark(_ id: UInt32) {
            guard !reachable.contains(id) else { return }
            reachable.insert(id)
            for child in children(of: id) { mark(child) }
        }
        mark(rootId)
        for (id, node) in built where !reachable.contains(id) {
            adapter?.destroy(node.view)   // adapt to the real destroy API
            forEachRowState.removeValue(forKey: id) // prune row bookkeeping too
            built.removeValue(forKey: id)
        }
```

   Adapt: `children(of:)`/`adapter?.destroy`/`forEachRowState` to the file's real accessors (`grep -n "func destroy\|destroy(" ShadowTreeReconciler.swift | head`, and the ForEach row-state dict name).
2. Test (`ShadowTreeLeakTests`): reconcile a list of 10 rows down to 2 rows across 100 mutations; assert `built.count` returns to the expected size (≤ row count + chrome) and `adapter.destroy` was called exactly (10−2)+… times in total — i.e., no growth after settle.

**VERIFY:** Swift host suite green.

**COMMIT:** `fix(H4): reconciler destroys unreachable subtrees and prunes stale ForEach row state`

---

### T-309 — thunkBlobs must merge, not replace [H12] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift`

`thunkBlobs = blobs` (~lines 145–151) replaces on every frame while the sibling `thunkHandlerToNode` merges (the code comment warns about exactly this) — after any hot reload all other thunks lose their bytecode.

Find the assignment and change it to a merge, mirroring the sibling's pattern (adapt to the real dictionary type):

```swift
        // Audit H12: replacing thunkBlobs on every frame dropped every OTHER
        // thunk's bytecode after a hot reload; merge like thunkHandlerToNode.
        for (handlerId, blob) in blobs {
            thunkBlobs[handlerId] = blob
        }
        // Blobs for handlers no longer present in the new frame are removed so
        // stale thunks cannot resurrect:
        let liveHandlerIds = Set(blobs.keys)
        thunkBlobs = thunkBlobs.filter { liveHandlerIds.contains($0.key) }
```

**VERIFY:** Swift host suite green; add a test: two thunks registered, one updated by a frame, assert the other's blob is still decodable.

**COMMIT:** `fix(H12): thunkBlobs merges per frame instead of replacing (stale-thunk loss)`

---

### T-310 — Malformed frames must surface errors, not DEBUG-NSLog [H5] — Danger: MEDIUM

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift`

Malformed frames are only DEBUG-NSLog'd (~lines 309–328, 548); `lastError`/`lastFluxError` are never set in Release, and `(try? Instruction.decode(bytecode)) ?? []` turns a corrupt handler into a silent empty program.

1. In the frame-parse catch block(s): set the executor's error surface. Locate `lastFluxError` (grep it) and, in the catch, assign a wire-kind FluxError (mirror how VM faults construct one — `grep -n "FluxError(" FluxExecutor.swift | head`), e.g.:

```swift
            catch {
                // Audit H5: silent catch hid malformed frames in Release.
                lastFluxError = FluxError(
                    kind: .invalidFrame,
                    message: "malformed frame: \(error)",
                    span: nil
                )
                continue
            }
```

   Adapt the initializer to the real type (`grep -n "struct FluxError" -A 10 *.swift | head -20`). Delete the bare `#if DEBUG NSLog` (or keep it as an additional log under the error assignment).
2. Replace `(try? Instruction.decode(bytecode)) ?? []` with explicit failure:

```swift
        guard let decoded = Instruction.decode(bytecode) else {
            // Audit H5: a corrupt handler must surface, not execute as an
            // empty program.
            lastFluxError = FluxError(kind: .invalidDispatch, message: "handler \(id) has undecodable bytecode", span: nil)
            return
        }
```

3. Verify the error overlay path (T-311) makes these visible.

**VERIFY:** Swift host suite green; add a test feeding a truncated frame and asserting `lastFluxError != nil`.

**COMMIT:** `fix(H5): malformed frames and undecodable handlers surface as FluxErrors`

---

### T-311 — Error overlay must observe executor faults [H10] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift`, `runtimes/ios/FluxHost/FluxApp/Sources/FluxApp/FluxAppMain.swift` (locate with `grep -rn "lastFluxError" runtimes/ios/ --include="*.swift" | grep -v FluxHost/Sources | head`)

`FluxAppMain` reads `executor.lastFluxError` but `FluxExecutor` is not `ObservableObject`, so SwiftUI never re-renders and the overlay generally never appears.

1. In `FluxExecutor.swift`: add `@Published`-compatible notification without forcing the whole class onto the main actor — the smallest correct change is a notification hook:

```swift
    /// Audit H10: the error overlay cannot observe a plain property. Fires
    /// whenever a new error is recorded so SwiftUI can re-render.
    var onError: ((FluxError) -> Void)?
```

   and at every `lastFluxError = ...` assignment add `onError?(err)` (route through a private `setFluxError(_:)` helper to keep it one line).
2. In `FluxAppMain`, make the overlay model an `ObservableObject` with `@Published var fluxError: FluxError?`, and in the executor setup wire `executor.onError = { [weak model] err in DispatchQueue.main.async { model?.fluxError = err } }`.

**VERIFY:** add a UI-less unit test asserting `onError` fires on a malformed frame (from T-310's test path).

**COMMIT:** `fix(H10): executor faults notify the SwiftUI error overlay`

---

### T-312 — HTTP transport must not block the main actor [H8 / D13] — Danger: HIGH ⚠ STOP-AND-ASK before reworking the transport protocol

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/HttpCapabilities.swift` (~73–93), `runtimes/ios/FluxHost/Sources/FluxHost/HttpCapabilities+Entries.swift` (~119–156), `runtimes/ios/FluxHost/Sources/FluxHost/IOSNativeCapabilityHost.swift` (~127–161, 193–219)

`URLSessionHttpTransport.request` blocks on a semaphore **on @MainActor** — the UI freezes for the whole network round-trip. `pushStatus()` and `biometricAuthenticate()` also block main.

**Step 1 — make the transport async.**
1. Find the protocol: `grep -n "protocol HttpTransport" -A 8 HttpCapabilities.swift`. Change `func request(...) -> HttpResponse` to `func request(...) async throws -> HttpResponse` (or `async -> HttpResponse`, matching the codebase's error style).
2. `URLSessionHttpTransport.request`: delete the semaphore/dispatch-block; `try await URLSession.shared.data(for: request)` and map to the existing `HttpResponse` type.
3. Update every caller (`grep -rn "\.request(" HttpCapabilities*.swift IOSNativeCapabilityHost.swift`): capability entries become `async` — the `CapabilityImpl` signature may need `async` too; if the registry's `CapabilityImpl` is synchronous by design, convert the Http entries to the Pending-cell pattern in step 2 instead of threading async.

**Step 2 — Pending-cell pattern (required if signatures stay sync).** `Http.get` writes a pending cell into the signal store (the FLUX-047 store the executor already polls — see T-302) and returns the cell placeholder immediately; the URLSession completion closure calls the existing `resolveCell` path. This is the same mechanism the Kotlin side uses (15 s timeout, error body `"{}"`), and it also fixes the Kotlin↔Swift drift D13. Implement: timeout via `URLSessionConfiguration.timeoutIntervalForRequest = 15` on the executor's session, error body `"{}"` + real status propagation (delete the hardcoded `statusCode: 200` at `HttpCapabilities+Entries.swift:140-145`).

**Step 3 — push/biometric:** same Pending-cell restructure in `IOSNativeCapabilityHost.pushStatus()`/`biometricAuthenticate()` (~lines 53–62, 310–312): return immediately; the dialog/notification completion resolves the cell. Never `DispatchSemaphore.wait()` on @MainActor — add a `// audit H8` comment and grep-verify: `grep -rn "DispatchSemaphore" runtimes/ios/FluxHost/Sources/` must return no hits when done.

**VERIFY:** Swift suite green; new test `HttpTransportDoesNotBlockMain`: dispatch `request` on `@MainActor`, assert the main queue processes another enqueued block while the (stubbed, delayed) transport is in flight.

**COMMIT:** `fix(H8/D13): async Http transport with pending-cell resolution; 15s timeout; real status codes; main-actor never blocks`

---

### T-313 — fileSystemWrite must persist the payload, not the debug description [H6] — Danger: MEDIUM

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/IOSNativeCapabilityHost.swift`

`fileSystemWrite` (~lines 193–219) persists `dataField.value.description` (e.g. `str(12)`) — write/read asymmetric, all non-integer file data corrupted.

1. Locate the write path (`grep -n "description" IOSNativeCapabilityHost.swift | head`).
2. Replace with a renderer that resolves strings via the string table and scalars via the same `renderForToString` used by the Text adapter (`grep -rn "renderForToString" runtimes/ios/FluxHost/Sources/FluxHost/ | head`), rejecting containers:

```swift
        // Audit H6: `value.description` is a DEBUG representation ("str(12)");
        // persist the real payload instead.
        switch dataField {
        case .str(let sid):
            payload = stringTable.resolve(sid)   // adapt to the real resolver
        case .int(let i): payload = String(i)
        case .float(let f): payload = String(f)
        case .bool(let b): payload = b ? "true" : "false"
        case .null: payload = ""
        default:
            throw CapabilityError.typeMismatch("fileSystemWrite requires a scalar or string payload")
        }
```

3. Test: write `"hello"` then read; assert round-trip equality (currently returns `str(...)` garbage).

**VERIFY:** Swift suite green.

**COMMIT:** `fix(H6): fileSystemWrite persists resolved payload values, not debug descriptions`

---

### T-314 — fileSignalID overflow + cell-space collision [H7] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/IOSNativeCapabilityHost.swift` (~310–312), cross-check `Registry.swift:258`

`fileSignalID = 900_000 + pathId` uses plain `+` on an unmasked FNV hash ⇒ arithmetic-overflow crash; values can also exceed the cell allocator's 1_000_000 ceiling and collide with awaited capability cells.

The registry-side copy (`Registry.swift:258`, verified) is already `&+` but unmasked. Fix BOTH sites identically:

```swift
    /// Audit H7: masked into a reserved range below the allocator's 1_000_000
    /// ceiling; `&+` prevents the trapping overflow on unmasked FNV hashes.
    private static func fileSignalID(_ pathID: UInt32) -> UInt32 {
        900_000 &+ (pathID % 90_000)
    }
```

   (900_000 + 89_999 = 989_999 < 1_000_000 — no collision with `allocateCell()`.)

**VERIFY:** Swift suite green; add a test `fileSignalID_staysBelowCellAllocatorCeiling` looping 1_000_000 path ids (sampled) asserting `< 1_000_000`.

**COMMIT:** `fix(H7): fileSignalID masked below the cell-allocator ceiling; &+ everywhere`

---

### T-313b — Prop thunks must run under the executor's permission checker [D23 / P2.31] — Danger: MEDIUM

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift` (~883–972)

Prop thunks run with `capRegistry: .dev` and `AllowAllPermissionChecker()` — any prop thunk can invoke any capability with full permissions. Also: thunk throws silently degrade to stale props.

1. `grep -n "AllowAllPermissionChecker\|capRegistry: .dev" ShadowTreeReconciler.swift | head` — thread the reconciler's owner executor's real `permissionChecker` and registry into `materializeProps` (add stored properties set at construction; the reconciler is created by the executor/app shell — pass `executor.permissionChecker` through).
2. On thunk throw: assign `executor.lastFluxError` (helper from T-310) instead of swallowing — stale props remain as the fallback VALUE, but the error is surfaced.

**VERIFY:** Swift suite green; test: prop thunk calling cap 14 without permission grant surfaces CAPABILITY_DENIED via `lastFluxError`.

**COMMIT:** `fix(D23): prop thunks execute under the real permission checker; thunk faults surface`

---

### T-315 — Telemetry must compile out of Release [H11] — Danger: LOW

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/Telemetry.swift`, `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift` (~600, 1413)

The header claims `#if DEBUG` compile-out but nothing is guarded; per-instruction telemetry + 16-register copies + ~10k WS frames per 100k-gas tap ship in Release.

1. Wrap the whole telemetry emit implementation in `Telemetry.swift` with `#if DEBUG` … `#endif` while keeping the public functions as no-op stubs in Release:

```swift
public enum FluxTelemetry {
    public static func record(_ event: TelemetryEvent) {
        #if DEBUG
        TelemetrySink.shared.record(event)
        #endif
    }
    // ... same pattern for every public entry point
}
```

2. In `FluxBytecodeVM.swift`, guard the two per-instruction emit sites (~600 and ~1413) with `#if DEBUG` around the call (or rely on the stub — prefer the stub-only approach to keep the hot path clean: DELETE the call sites and keep `FluxTelemetry.record` calls only where already cheap; log which you chose).
3. Test: build scheme in Release configuration (`xcodebuild build -scheme FluxHost -configuration Release`) succeeds; Debug suite unchanged.

**VERIFY:** `cd runtimes/ios/FluxHost && xcodebuild build -scheme FluxHost -configuration Release -destination 'platform=macOS' 2>&1 | tail -3`

**COMMIT:** `fix(H11): telemetry emits only in DEBUG builds per its documented contract`

---

### T-316 — Swift correctness sweep (smaller items, batched) — Danger: MEDIUM

Execute in order. VERIFY after each: Swift host suite green.

| ID | Finding | File(s) | Instruction |
|---|---|---|---|
| T-316.1 | H9 store/commit races | `FluxExecutor.swift` (~309–328, 548, 461–531, 589–675) | `runHandlerAsync` works on `var store = graph` and commits `graph = store` at HALT; two interleaved dispatches lose each other's signal writes. Minimal deterministic fix: serialize dispatches — add a `isDispatching` flag + pending queue; a dispatch arriving while one runs is queued, not run concurrently. (The delta-merge alternative is better long-term; log it as SIDECAR.) |
| T-316.2 | P2.26 dispatch table gaps | `FluxBytecodeVM.swift` (~1152–1417) | `runViaDispatchTable`: add the FLUX-049 permission gate to its CALL_CAP arm (mirror the main path), replace `default: fatalError` for LIST_*/IS_NULL/AWAIT with the same implementations as the main path, or delete the public entry point. Prefer: delete if tests-only (`grep -rn "runViaDispatchTable" runtimes/ios` — if only tests call it, mark `internal` and reuse the main loop instead). |
| T-316.3 | P2.27 JSON→record key bug | `FluxValueJsonParser.swift` (~43–53) AND Kotlin twin `runtimes/android/host/src/main/kotlin/dev/flux/host/vm/FluxValueJson.kt` (~60–70) | JSON objects parse to records whose field VALUES are the interned KEY names; actual values discarded → `Http.getJson` cannot return data. Fix BOTH sides together: for each JSON member, the record's field PropIdx comes from the key's `prop_index_for_name(key)` and the VALUE from the member value. Add a shared fixture `tests/isa-vectors/json_object.json` (`{"a": 1, "b": "x"}` → record fields `[("a",Int 1),("b",Str "x")]`) and unit tests on both platforms reading it. |
| T-316.4 | P2.28 hand-built test frames omit kind byte | `WireDecodeTests.swift` (~76–169), `DeserializeAllocPerfTests.swift` (~40) | Update the hand-built frames to include the 1-byte kind field (mirror the Rust encoder layout in `crates/flux-ir-serde/src/frame.rs`). The tests must go green against the real decoder — if a test still fails, its layout was wrong, fix the TEST. |
| T-316.5 | P2.29 WS reconnect + appendLog | `FluxWebSocketTransport.swift` (~102–147) | Add a `userInitiatedClose` flag set in `close()`; `handleDrop` must not schedule reconnect when it is set. Replace `appendLog` (unbounded file growth, force-unwrap) with `os_log` (Logger). |
| T-316.6 | P2.30 handlerCount ignored | `FrameDeserializer.swift` (~186, 200) | After parsing `handlerCount`, gate the blob-section read on it (or `assert(handlerCount == computed)` when the section is present); never read unconditionally. |
| T-316.7 | P2.33 crash handlers | `CrashReporter.swift` (~11) | Remove the RELEASE-TODO by installing the handlers in the app entry (`FluxAppMain`): `NSSetUncaughtExceptionHandler` + a SignalExceptionHandler that forwards to `CrashReporter.record`. |
| T-316.8 | P2.34 fatal on bad URL | `FluxAppMain.swift` (~73–75) | Replace `fatalError` on malformed `FLUX_WS_URL` with: fall back to loopback default + surface a banner via the T-311 error channel. |
| T-316.9 | P2.35 span mapping | `FluxExecutor.swift` (~197, 209) | Server spans are byte offsets (`start`, `end`); stop mapping them into `SourceSpan(fileID:line:column:)`. Store raw offsets and convert to line/col at display time using the file text (or display `offset start..end` until T-608's shared converter exists). |
| T-316.10 | D20 ScrollView | `adapters/ui-swift/Sources/FluxUIKit/ScrollViewAdapter.swift` | `setChildren` grabs `subviews.first` then removes ALL subviews including its own content host → blank scroll view; `orientation` never applied. Fix: keep the content host view; swap only its arranged subviews; apply `orientation` (horizontal ⇒ `alwaysBounceHorizontal` + axis-constrained layout, mirroring Kotlin's `LinearAdapters.kt` ScrollView handling). Test: two consecutive `setChildren` calls leave the scroll view functional (content host identical instance). |

**COMMIT (one per item):** `fix(<ID>): <one-liner>`.

---

## 3B. Kotlin Android host

---

### T-330 — ForEach reconcile must use ONE id space end-to-end [C12] — Danger: HIGH

**Files:** `runtimes/android/host/src/main/kotlin/dev/flux/host/shadow/ShadowTree.kt`

`desired` is built from raw `deriveForEachRowId(foreachId, i)` ids, but row `ShadowNode.id`s live in the `deriveForEachChildId(rowId, …)` space (the cloned wire root id is `deriveForEachChildId(rowId, template.id)`). Consequences: teardown's `child.id !in desired` is ALWAYS true (every reconcile destroys and rebuilds all rows), and from the 2nd list mutation `nodes[rowId]` returns the just-destroyed node, so `adapter.update()` runs on a destroyed view.

The canonical row-root id is `rowRootId(foreachId, i) = deriveForEachChildId(deriveForEachRowId(foreachId, i), template.id)`.

In `reconcileForEach` (verified lines 858–924), find the desired-building block:

```kotlin
        val desired =
            list.items.mapIndexed { i, elem ->
                val rowId = deriveForEachRowId(foreachId, i.toUInt())
                val childIds = linkedSetOf<UInt>()
                expandedIndex[rowId] = cloneWireNode(template, rowId, childIds, wireIndex)
                if (templateMeta != null) signalMetaOverride[rowId] = templateMeta
                elem?.let { element ->
                    childIds.forEach { forEachRowContext[it] = itemSlot to element }
                    forEachRowContext[rowId] = itemSlot to element
                }
                rowId
            }.toSet()
```

Replace with (uses the canonical id everywhere — `expandedIndex`, `signalMetaOverride`, `forEachRowContext`, `desired`):

```kotlin
        // Audit C12: `desired` must be expressed in the SAME id space as the
        // built rows (`deriveForEachChildId`), otherwise teardown destroys
        // every row on every reconcile and `nodes[rowId]` later resolves to a
        // destroyed node. Canonical row-root id = childId(rowId, template.id).
        val desired = linkedSetOf<UInt>()
        list.items.forEachIndexed { i, elem ->
            val rowId = deriveForEachRowId(foreachId, i.toUInt())
            val rootId = deriveForEachChildId(rowId, template.id)
            val childIds = linkedSetOf<UInt>()
            expandedIndex[rootId] = cloneWireNode(template, rowId, childIds, wireIndex)
            if (templateMeta != null) signalMetaOverride[rootId] = templateMeta
            elem?.let { element ->
                childIds.forEach { forEachRowContext[it] = itemSlot to element }
                forEachRowContext[rootId] = itemSlot to element
            }
            desired.add(rootId)
        }
```

Then in the build loop below (verified lines 888–903), find:

```kotlin
        val newChildren =
            list.items.mapIndexed { i, elem ->
                val rowId = deriveForEachRowId(foreachId, i.toUInt())
                val child =
                    nodes[rowId] ?: run {
                        val childIds = linkedSetOf<UInt>()
                        val wire = expandedIndex[rowId]
                            ?: cloneWireNode(template, rowId, childIds, wireIndex)
```

Replace with:

```kotlin
        val newChildren =
            list.items.mapIndexed { i, elem ->
                val rowId = deriveForEachRowId(foreachId, i.toUInt())
                val rootId = deriveForEachChildId(rowId, template.id)
                val child =
                    nodes[rootId] ?: run {
                        val childIds = linkedSetOf<UInt>()
                        val wire = expandedIndex[rootId]
                            ?: cloneWireNode(template, rowId, childIds, wireIndex)
```

and two lines below, find:

```kotlin
                        val built = build(wire, wireIndex, host, 1u)
                        nodes[rowId] = built
                        parents[rowId] = foreachId
                        built
```

Replace with:

```kotlin
                        val built = build(wire, wireIndex, host, 1u)
                        nodes[rootId] = built
                        parents[rootId] = foreachId
                        built
```

Sanity-check the rest of the function (lines 904–923) for any remaining `rowId` used as a KEY into `nodes`/`parents`/`reconciled` — every key must be `rootId`. The teardown block (lines 879–885) needs no change once `desired` uses `rootId` — it already compares `child.id` (built ids) against `desired`.

**Tests** — add to `runtimes/android/host/src/test/kotlin/dev/flux/host/ForEachReconcileIdSpaceTest.kt` (new file; reuse the harness from `ForEachRemoveBugTest.kt`):

```kotlin
@Test fun `two consecutive list mutations never update a destroyed view`() {
    // add row -> remove row -> add row; assert no adapter.update() is invoked
    // on a view whose destroy() was already called (tracking adapter).
}
@Test fun `reconcile reuses row identity when list is unchanged`() {
    // two deltas with the same list; assert adapter.destroy call count == 0
    // and the same view instances are reused.
}
```

**VERIFY**
```bash
cd runtimes/android && ./gradlew :host:testDebugUnitTest --console=plain 2>&1 | tail -5
```

**COMMIT:** `fix(C12): ForEach reconcile uses the derived-child id space for desired/nodes/parents`

---

### T-331 — Unify ForEach row-id derivation and honor splice keys [D1 / D2] — Danger: HIGH ⚠ STOP-AND-ASK (cross-platform contract change)

**Files:** `runtimes/android/host/src/main/kotlin/dev/flux/host/shadow/ShadowTree.kt` (~927–929), `runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift` (~646–667), new shared vector file `tests/isa-vectors/foreach_ids.json`

Kotlin and iOS use different hash schemes; Android's unmarked derived ids can collide with real server node ids. Both key rows by index and discard the wire splice `key: UInt64`. This task adopts ONE collision-safe scheme (Swift's FNV + marker bits) and makes splice keys the row identity.

**Step 1 — single source of truth.** Create `tests/isa-vectors/foreach_ids.json` documenting the normative algorithm:

```json
{
  "row_id":    "fnv1a64_mix: start=0x811c9dc5; foreach_id bytes LE; 0x2C marker byte; index_or_key u64 LE; multiply 0x01000193 after each byte",
  "child_id":  "fnv1a64_mix: start=0x811c9dc5; row_id u64 LE; 0x3F marker byte; orig_id u64 LE; multiply 0x01000193 after each byte",
  "markers":   { "row": "0x8000_0000", "child": "0xC000_0000", "note": "final u32 result ORed with the marker; truncates to UInt32" },
  "vectors": [
    { "input": { "foreach_id": 20, "row_index": 1, "orig_id": 10 }, "row_id": "<fill by running the reference impl>", "child_id": "<fill>" }
  ]
}
```

   Generate the `vectors` by writing ONE reference implementation as a Rust test in `crates/flux-syntax/tests/foreach_ids.rs` (the compiler is the neutral party), running it, and pasting the printed values into the JSON. Commit the Rust test — it is the generator.

**Step 2 — Kotlin adopts the scheme.** Replace `deriveForEachRowId`/`deriveForEachChildId` (verified at lines 927–929) with implementations matching the JSON exactly (FNV-1a per byte with `toUByte`-based hashing — this ALSO closes P2.36's two-FNV disagreement; keep one shared private `fnv1a(bytes: ByteArray): UInt` helper in this file and reuse it from `PropsIndex` usage sites). OR the marker bits: `id or 0x8000_0000u` for rows, `id or 0xC000_0000u` for children.

**Step 3 — splice keys become row identity (both platforms).**
- Kotlin: in `reconcileForEach` (from T-330), when the wire splice carries a `key` (the ForEach metadata exposes it — `grep -n "spliceKey\|key" ShadowTree.kt | head -20` and `WireNode`/splice decoding), derive the row id from `fnv(foreach_id, key)` instead of the index. Fall back to the index only when no key was provided. Keep u64 keys until the hash input (this closes D10's u32 truncation at the same time).
- Swift: same change at `ShadowTreeReconciler.swift:646-667`.
- Both: read the vector file in a unit test (`ForEachIdVectorsTest.kt`, `ForEachIdVectorsTests.swift`) asserting identical ids for identical inputs.

**Step 4 — collision guard test (both platforms):** a derived row id must never equal a real server node id: assert `derivedId and 0xC000_0000u != 0u` for every derived id in the vectors.

**VERIFY**
```bash
cargo test -p flux-syntax foreach_ids 2>&1 | tail -3
cd runtimes/android && ./gradlew :host:testDebugUnitTest --console=plain 2>&1 | tail -5
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | tail -5
```

**COMMIT:** `fix(D1/D2): one FNV+marker row/child id scheme from shared vectors; splice keys are row identity`

---

### T-332 — Kotlin Color/Font record decoding must be positional [D5] — Danger: MEDIUM

**Files:** `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/Props.kt` (~58–76), `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/PropsIndex.kt` (read-only reference)

The server lowers `Color = RGB(Float,Float,Float)` / `Font(...)` **positionally** (Swift reads fields 0,1,2,3 / 0,1,2 — `Color.swift` r=0,g=1,b=2,a=3; `TextAdapter.swift` size=field 1). Kotlin looks the fields up by FNV-hashed names (`PropsIndex.COLOR_RED`, …) and therefore silently drops every color/font from the server.

In `Props.kt`, find (verified lines 57–76):

```kotlin
    /** Decodes the `Color` record at [index] into a [FluxColor], or `null`. */
    public fun getColor(index: UShort): FluxColor? {
        val record = getRecord(index) ?: return null
        val red = record.getFloat(PropsIndex.COLOR_RED) ?: return null
        val green = record.getFloat(PropsIndex.COLOR_GREEN) ?: return null
        val blue = record.getFloat(PropsIndex.COLOR_BLUE) ?: return null
        val alpha = record.getFloat(PropsIndex.COLOR_ALPHA) ?: 1.0
        return FluxColor(red, green, blue, alpha)
    }

    /** Decodes the `Font` record at [index] into a [FluxFont], or `null`. */
    public fun getFont(index: UShort): FluxFont? {
        val record = getRecord(index) ?: return null
        val size = record.getFloat(PropsIndex.FONT_SIZE) ?: return null
        return FluxFont(
            size = size,
            weight = record.getString(PropsIndex.FONT_WEIGHT),
            family = record.getString(PropsIndex.FONT_FAMILY),
        )
    }
```

Replace with:

```kotlin
    // Audit D5: the server lowers Color/Font records POSITIONALLY (fields
    // 0..3 / 0..2). The previous FNV-name lookups never matched, so colors
    // and fonts were silently dropped on Android. Field order mirrors
    // adapters/ui-swift Color.swift + TextAdapter.swift.

    /** Float value at the record's positional [slot], or `null`. */
    private fun FluxValue.Record.floatAt(slot: Int): kotlin.Double? =
        (fields.getOrNull(slot)?.value as? FluxValue.Float)?.value

    private fun FluxValue.Record.stringAt(slot: Int): String? =
        (fields.getOrNull(slot)?.value as? FluxValue.Str)?.value

    /** Decodes the `Color` record at [index] into a [FluxColor], or `null`. */
    public fun getColor(index: UShort): FluxColor? {
        val record = getRecord(index) ?: return null
        val red = record.floatAt(0) ?: return null
        val green = record.floatAt(1) ?: return null
        val blue = record.floatAt(2) ?: return null
        val alpha = record.floatAt(3) ?: 1.0
        return FluxColor(red, green, blue, alpha)
    }

    /** Decodes the `Font` record at [index] into a [FluxFont], or `null`. */
    public fun getFont(index: UShort): FluxFont? {
        val record = getRecord(index) ?: return null
        val size = record.floatAt(0) ?: return null
        return FluxFont(
            size = size,
            weight = record.stringAt(1),
            family = record.stringAt(2),
        )
    }
```

   Mechanically: `fields` is the record's field list — confirm the accessor names on `FluxValue.Record` (`grep -n "class Record" -A 12 runtimes/android/host/src/main/kotlin/dev/flux/host/vm/FluxValue.kt adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/*.kt 2>/dev/null | head -30`); the kit's `Props` layer receives `FluxValue` from the host module — adapt the helper to whatever record type `getRecord` returns. Channel clamping: mirror Swift's `min(max(v, 0), 1)` in `FluxColor`'s init (`adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxStyle.kt`).

Port the decode tests from Swift (closes the D24 Kotlin-side gap): add `adapters/ui-kotlin/src/test/kotlin/dev/flux/ui/ColorFontDecodeTest.kt` with: full record → color; partial record → null; out-of-range channel → clamped; font size field 1.

**VERIFY**
```bash
cd runtimes/android && ./gradlew :host:testDebugUnitTest :app:assembleDebug --console=plain 2>&1 | tail -5
```

**COMMIT:** `fix(D5): Kotlin decodes Color/Font records positionally matching the server and Swift`

---

### T-333 — Swift signal graph must batch like Kotlin [D7] — Danger: HIGH

**Files:** `runtimes/ios/FluxHost/Sources/FluxHost/SignalGraph.swift` (~117–129)

Swift notifies synchronously inside `write` (intermediate states visible); Kotlin batches with `pending` + `flush()` in id order (Android is the reference — port its model).

1. Read the reference: `runtimes/android/host/src/main/kotlin/dev/flux/host/signal/SignalGraph.kt` — note its `pending` set + ordered `flush()`.
2. Port to Swift: buffer writes into `pendingWrites: [UInt32: Value]` inside `write`; add `func flush()` that applies in ascending signal-id order and notifies subscribers once per signal. All internal call sites that previously relied on synchronous propagation must call `flush()` after a batch boundary (handler dispatch end, reconciler prop materialization end — grep `graph.write` call sites and wrap the batch boundaries: `grep -rn "\.write(" runtimes/ios/FluxHost/Sources/FluxHost/*.swift | head -20`).
3. Test: two signals where the second's observer reads the first — intermediate writes are NOT observable mid-batch; final state matches Kotlin's (port the Kotlin batching test as `SignalGraphBatchTests`).

**VERIFY:** Swift suite green.

**COMMIT:** `fix(D7): Swift signal graph batches writes and flushes in id order (Kotlin parity)`

---

### T-334 — Drift-matrix policy unification (D8–D19) — Danger: MEDIUM each

One PR-sized change each; the "Fix side" column says WHICH platform changes. Each item ends with a one-line encoding note to append to `docs/appendix-f-parity.md` (create the file if absent; this becomes the shared behavioral contract).

| ID | Drift | Fix instruction |
|---|---|---|
| T-334.1 | **D8** handshake versions | Kotlin `runtimes/android/host/src/main/kotlin/dev/flux/host/wire/FrameDeserializer.kt` (~43): remove `0x01` from the supported set (`setOf(...)`), keep v2 only. If a `WireFixtureContractTest` covers v1, update it. |
| T-334.2 | **D9** malformed node-kind | Both decoders: on unknown node kind, degrade per-node (skip the node, emit a diagnostic count) instead of failing the frame — iOS `FrameDeserializer.swift` change; add the same count to the Kotlin path's existing behavior. Encode: "per-node degrade + error surface". |
| T-334.3 | **D10** splice key width | Folded into T-331 step 3 (Kotlin keeps u64). Mark DONE if T-331 executed. |
| T-334.4 | **D11** presence flags | Both `FrameDeserializer`s: presence is `!= 0` (iOS `== 1` is the deviant; grep the flag decoding and normalize). Test: a `2` in a presence byte decodes as present on both. |
| T-334.5 | **D12** StorageBackend | iOS `StorageBackend.swift`: adopt Kotlin's behavior — atomic rename on write (write temp + `FileManager.replaceItem`/rename), corrupt entry quarantined (deleted) on read, NaN rejected on write. Mirror Kotlin `StorageBackend.kt`. |
| T-334.6 | **D13** HTTP defaults | Folded into T-312. Mark DONE if executed. |
| T-334.7 | **D14** absent-prop policy | BOTH kits: an absent prop RETAINS the previous value (do not reset to `""`/0). Kotlin `TextAdapter.kt`/`ButtonAdapter.kt` etc. reset on absent — remove the reset branches (grep `?: ""` and `?: 0.0` inside `update(` methods of `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/*.kt`). Encode in Appendix F: "absent prop = retain". |
| T-334.8 | **D15** gesture default kind | Both kits: a missing/unknown gesture `kind` is an error surfaced to the overlay, NOT a default recognizer (iOS currently attaches `longPress`). |
| T-334.9 | **D16** Stack semantics | iOS `StackAdapter.swift`: render z-overlay (children stacked), matching Kotlin `LinearAdapters.kt` STACK handling and stdlib docs. Test: two children overlap on iOS. |
| T-334.10 | **D17** error overlay | Port Android's ADR-0057 overlay (snippet + caret + tiers, `runtimes/android/app/src/main/kotlin/dev/flux/app/ErrorOverlay.kt`) to iOS `FluxAppMain`; both platforms map byte-offset spans to line:col (shared helper duplicated per platform; unit test both). |
| T-334.11 | **D18** WebHost missing src | iOS `WebHostAdapter.swift`: absent `src` clears the view; keep the http/https scheme check but move the policy decision into a shared helper comment so both kits agree ("non-http(s) src ⇒ error surface"). |
| T-334.12 | **D19** TextInput placeholder/caret | iOS `TextInputAdapter.swift`: (a) do not clear the placeholder on absent (retain semantics, D14); (b) remove `textFieldDidChangeSelection` observation — caret moves must NOT fire `onChangeText`. |

**VERIFY:** each item — both platform suites green.

**COMMIT:** `fix(Dxx): <one-liner> (parity policy, Appendix F updated)`.

---

### T-335 — Android/Swift smaller host fixes (batched) — Danger: LOW/MEDIUM

| ID | Finding | File(s) | Instruction |
|---|---|---|---|
| T-335.1 | H23 STR_LEN | `crates/flux-vm-ref/src/vm.rs` (~571–574) | `STR_LEN` currently returns the digit count of the string **id** (panics via `u32::ilog10(0)` on id 0). Return the real byte length from the string table (`self.strings.resolve(id).len()` — adapt to the VM's table API). Add a vector to `tests/isa-vectors/typed_arith.json`. |
| T-335.2 | H23 CALL_CAP stubs | `crates/flux-vm-ref/src/vm.rs` (~706) | `CALL_CAP` hardwires `CapabilityRegistry::with_parity_stubs()`; thread a registry through `Vm::new(...)` (optional param defaulting to the stubs), and have conformance tests inject real fakes. No behavior change for existing callers. |
| T-335.3 | P2.36 two FNVs on Android | `ShadowTree.kt` (~1183) vs `PropsIndex.kt` | Folded into T-331 step 2 (single `fnv1a` helper). Mark DONE if executed; else extract the shared helper now and point both call sites at it. |
| T-335.4 | P2.37 dead reactive layer | `shadow/DirtyReconciler.kt`, `signal/SignalGraph.kt` | `observe/invalidate` have zero callers: delete them AND their tests, or document the intended use in the class KDoc with a tracking issue reference. Prefer deletion (dead code in a runtime is a liability). |
| T-335.5 | P2.38 cleartext traffic | `runtimes/android/app/src/main/AndroidManifest.xml` (~12) | Remove `android:usesCleartextTraffic="true"`; add `android:networkSecurityConfig="@xml/network_security_config"` and create `runtimes/android/app/src/main/res/xml/network_security_config.xml` allowing cleartext ONLY to `127.0.0.1` and `10.0.2.2` (dev loop). |
| T-335.6 | P2.39 publish AAR | `.github/workflows/artifact-publish.yml` (~10–11 vs 87–98) | Header says host AAR; job publishes a debug APK. Change the job to `./gradlew :host:publishToMavenLocal` (or `:host:assembleRelease`) and upload `host/build/outputs/aar/*.aar` (or the published artifact); fix the header text. If the module cannot produce a release AAR yet, keep the APK but fix the header AND rename the artifact `flux-host-debug.apk`, logging `T-335.6 PARTIAL`. |
| T-335.7 | H26 release build hardening | `runtimes/android/app/build.gradle.kts` (~33–37) | Set `isMinifyEnabled = true`, `isShrinkResources = true`, add `proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")`, create `runtimes/android/app/proguard-rules.pro` with keep rules for OkHttp (`-keep class okhttp3.** { *; }`, `-dontwarn okhttp3.**`) and Compose defaults, and add a `signingConfigs.release` block reading `FLUX_SIGNING_STORE_FILE`/`STORE_PASSWORD`/`KEY_ALIAS`/`KEY_PASSWORD` env vars with a debug fallback to `signingConfig signingConfigs.debug`. VERIFY: `./gradlew :app:assembleRelease` succeeds and output is not debuggable (`aapt dump badging` or check `AndroidManifest` merged). |
| T-335.8 | D6 alignment encoding | `stdlib/*.flux`, `crates/flux-ir`, both kits | Stdlib declares an ADT the IR can't encode (`lower/mod.rs:934,940` lowers variant idents to Null). Pick ONE encoding: record `{0: int}` where 0=start,1=center,2=end (mirrors Color's positional record). Implement: stdlib `alignment` typed as that record; Kotlin `TextAdapter.kt:42` + `LinearAdapters.kt` STACK_ALIGNMENT read the int; iOS `TextAdapter.swift:35`/`ColumnAdapter.swift:37` switch to the same ints. Update `docs/appendix-f-parity.md`. |
| T-335.9 | D21/D22/LOW batch | misc | Encode in Appendix F only: image cache = memory-only LRU is canonical (Kotlin), non-canonical string-id reserved ranges: Kotlin fallback ids ≥ 0x8000_0000, iOS local interning ≥ 0xC000_0000 (documented, no code change). |

**VERIFY:** combined run of both platform suites + `cargo test -p flux-vm-ref`.

**COMMIT:** one per row: `fix(<ref>): <one-liner>`.

---

### T-336 — Phase 3 exit gate

- All three suites green (`cargo`, `gradlew :host:testDebugUnitTest`, `xcodebuild FluxHost`).
- `tests/isa-vectors/` contains `list_ops.json`, `foreach_ids.json`, `json_object.json`, `typed_arith.json` — each read by at least one test on each applicable platform.
- Leak test (T-308) and double-mutation test (T-330) present and green.
- `docs/appendix-f-parity.md` exists and records D8–D19 + D21/D22 decisions.
- Grep gates: `grep -rn "DispatchSemaphore" runtimes/ios/FluxHost/Sources/` → empty; `grep -n "removeAll()" runtimes/ios/FluxHost/Sources/FluxHost/ShadowTreeReconciler.swift` → only the guarded root-Replace site.

# PHASE 4 — RELEASE CODEGEN: SAME SOURCE, COMPILING APPS ON BOTH PLATFORMS

**Goal:** `flux build` output compiles for the stdlib examples on both toolchains; no generated-code injection via strings.
**Exit criteria:** CI step compiles examples/counter, examples/todo, examples/router output with Swift (swiftc) and Kotlin (kotlinc/gradle) — or records the missing toolchain per R6.

---

### T-401 — String escaping for generated code (both backends) [H21 / §5.10] — Danger: HIGH

**Files:** `crates/flux-codegen-core/src/expressions.rs` (~68–82), call sites in `crates/flux-codegen-kotlin/src/`, `crates/flux-codegen-swift/src/`

`render_string` does **no escaping at all** — `Text("say \"hi\"")` emits invalid code, and Kotlin additionally breaks on `$` (the repo's own scaffold `"tapped $${count} times"` is an immediate Kotlin compile error).

1. In `expressions.rs`, find `render_string` and its callers (`grep -rn "render_string" crates/flux-codegen-core/src crates/flux-codegen-kotlin/src crates/flux-codegen-swift/src | head`). Replace the single shared renderer with a per-backend escape hook:

```rust
/// Escapes user text for embedding inside a generated string literal.
/// Each backend has different metacharacters; the shared renderer used to
/// emit raw text, which produced non-compiling (or injectable) output
/// (audit H21).
pub trait TextEscaper {
    fn escape(&self, s: &str) -> String;
}

/// Swift: escape backslash, double quote, and newlines.
pub struct SwiftEscaper;
impl TextEscaper for SwiftEscaper {
    fn escape(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
        }
        out
    }
}

/// Kotlin: Swift's rules PLUS `$` (string templates) and backtick.
pub struct KotlinEscaper;
impl TextEscaper for KotlinEscaper {
    fn escape(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '$' => out.push_str("\\$"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
        }
        out
    }
}
```

2. `render_string` gains a `&dyn TextEscaper` parameter; the Kotlin backend passes `KotlinEscaper`, Swift passes `SwiftEscaper` (thread from each backend's emitter entry — follow the compiler errors).
3. Tests: `crates/flux-codegen-core/tests/escaping.rs` — `Text("say \"hi\"")` round-trips in both backends' output; the scaffold fixture string `tapped $${count} times` produces Kotlin output containing `\\$`; a string containing `"){ … }` cannot terminate the literal early.

**VERIFY**
```bash
cargo test -p flux-codegen-core 2>&1 | tail -3
```

**COMMIT:** `fix(H21): per-backend string escaping in release codegen (Swift + Kotlin/$ rules)`

---

### T-402 — Kotlin backend: prelude, ForEach, animation [H22 / §5.2, §5.3, §5.6] — Danger: HIGH

**Files:** `crates/flux-codegen-kotlin/src/backend_impl.rs` (~97–98, 193–195, 197–217), `crates/flux-codegen-core/src/emitter.rs` (~471–480)

1. **Prelude imports.** In the Kotlin prelude (search `grep -n "import\|PRELUDE" crates/flux-codegen-kotlin/src/backend_impl.rs | head`), ensure ALL of these are emitted at the top of generated files (the generated output references them): `androidx.compose.foundation.Image`, `androidx.compose.ui.res.painterResource`, the generated `items` accessor context, `androidx.compose.animation.core.*`, `kotlinx.coroutines.GlobalScope`, `androidx.compose.foundation.shape.RoundedCornerShape`. Add exactly the missing six per the audit; re-derive by compiling a sample (step 4).
2. **ForEach → LazyColumn.** In the ForEach emission (~97–98), the Kotlin path emits `items(...)` inside a plain `Column` — only valid in `LazyListScope`. Emit `LazyColumn { items(...) }` for ForEach bodies in Kotlin (Swift keeps `ForEach(c, id: \.id)` — this asymmetry is fine; the backends emit different but valid containers). Log a SIDECAR row if a `Row`-with-ForEach combination needs `LazyRow` — implement `LazyRow` symmetrically when the parent container is a horizontal Row.
3. **withAnimation.** Kotlin has no `withAnimation`. Map the shared `Animate` emission: Kotlin uses `animate*AsState`/`Animatable` (e.g. `val animated = animateFloatAsState(targetValue = v, animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy))`); map curve names `bouncy`→`spring(dampingRatio=0.5f)`, `smooth`→`tween(300)`. In `emitter.rs` (~471–480), split the shared Toggle/animate emission into backend hooks if not already (see T-403).
4. **Compile check (the real gate).** Emit Kotlin for `examples/counter` and `examples/todo` (via `flux build --target android` or the codegen test harness in the crate) and compile with `kotlinc` if installed; otherwise add a unit test asserting the output contains no `withAnimation(`, contains `LazyColumn`/`LazyRow` for ForEach, and imports list includes every referenced symbol (a "referenced symbol ∈ imports" check: regex every `Image(`, `painterResource(`, `GlobalScope`, `RoundedCornerShape(`, `animateFloatAsState(` occurrence against the import block).

**VERIFY**
```bash
cargo test -p flux-codegen-kotlin 2>&1 | tail -3
```

**COMMIT:** `fix(H22): Kotlin prelude imports, LazyColumn ForEach, animate*AsState mapping`

---

### T-403 — Backend-split codegen fixes (Toggle, guard arms, TextField, router, spacing, header) [§5.1, §5.4–§5.9] — Danger: HIGH

**Files:** `crates/flux-codegen-core/src/emitter.rs`, `crates/flux-codegen-kotlin/src/backend_impl.rs`, `crates/flux-codegen-swift/src/backend_impl.rs` (or the crate's real file layout — `ls crates/flux-codegen-swift/src`)

Execute each sub-item; each is one generated-code shape change:

| ID | Item | Instruction |
|---|---|---|
| T-403.1 | §5.1 Toggle | Shared emitter hardcodes Swift `Toggle(isOn: .constant(v)) {}` for both backends (~`emitter.rs:471-480`). Add a backend hook: Swift keeps `Toggle(isOn: <binding>) { … }`; Kotlin emits `Switch(checked = <state>, onCheckedChange = { <write> })`. |
| T-403.2 | §5.4 Button styles | Cupertino→`.bordered` has no Material equivalent: Kotlin emits plain `Button` (distinction is acceptable); Swift keeps `.bordered`/`.borderedProminent`. Ensure the shared emitter calls a `button_style` backend hook instead of emitting Swift modifiers into Kotlin output. |
| T-403.3 | §5.5 spacing axis | Kotlin always emits `verticalArrangement` — for a Row use `horizontalArrangement`. Detect the parent container axis in the emitter and emit the correct one. |
| T-403.4 | §5.6 component header | Kotlin emits leading commas (`    , other: String`) and no `data object` for zero-field variants. Fix the parameter-list join to `, `-separated, and emit `data object X` (not `data class X`) for variants with no fields. |
| T-403.5 | §5.7 guard/match arms | Guard pattern → Swift `default:` (swallows later arms) vs Kotlin `is Type ->` (exact). Make BOTH backends emit the exact-type test first and the catch-all LAST: Swift `if case .guardType = x { … } else if …` or `switch` with explicit case + `default` at the end; Kotlin keeps `is Type ->`. Add a codegen test: source with guard-arm followed by a variant arm produces arms in source order on both backends. |
| T-403.6 | §5.8 TextField | Swift emits `TextField("", text: .constant(v), onEditingChanged: …)` — a Bool callback with a constant binding = read-only field. Emit `TextField("", text: $state)` with the state actually mutable (generate a `@State private var` for the bound signal in the component body). |
| T-403.7 | §5.9 Router | Swift hijacks ANY state named `route` into `NavigationPath()` regardless of declared type; Kotlin hardcodes `startDestination = "home"`. Swift: only treat `route` as navigation state when its declared type is the router's route type (check the component's state table); Kotlin: derive `startDestination` from the declared route constant/default, not the literal `"home"`. |
| T-403.8 | §5.11 async | `render_handler_body` emits Swift `await` verbatim inside Kotlin `GlobalScope.launch { … }`. Strip/translate: Kotlin launches a coroutine — replace `await expr` with `expr.await()` inside the coroutine (and ensure `kotlinx.coroutines` import), or emit `runBlocking`-free suspend style per the file's existing convention. The output must not contain a bare ` await ` token outside comments (test asserts this). |

**VERIFY**
```bash
cargo test -p flux-codegen-core 2>&1 | tail -3
cargo test -p flux-codegen-kotlin 2>&1 | tail -3
cargo test -p flux-codegen-swift 2>&1 | tail -3
```

**COMMIT:** one per row: `fix(§5.x): <one-liner>`.

---

### T-404 — CLI build output: one file per source, deterministic entry point [P2.23] — Danger: MEDIUM

**Files:** `crates/flux-cli/src/build.rs` (~95–108, ~140–148), `crates/flux-cli/src/sources.rs` (~36–51), `crates/flux-cli/src/init.rs` (~31–32)

1. Android output writes EVERY compiled source to the constant `"MainActivity.kt"` in one loop — multiple `.flux` files silently overwrite each other. Fix the loop to derive the filename from the source file stem: `format!("{stem}.kt")` (stem = source file name without `.flux`, sanitized to PascalCase), and keep `MainActivity.kt` reserved for the entry file that hosts `MainActivity`.
2. Entry point = last-declared component — App-first layouts launch the wrong screen. Change selection to: the component named `App` if present, else the FIRST declared component; log a warning when falling back.
3. `sources.rs` skips unreadable files with a warning — make it a hard error (a silently dropped source produces a broken app).
4. `init.rs` scaffold uses `onClick:` which `collect_handler` silently drops (dev increments, release does nothing). Change the scaffold to the verb the collector recognizes (`grep -n "onPress\|onClick\|collect_handler" crates/flux-parser/src crates/flux-ir/src | head` — use whatever event-verb the lowering accepts, per the T-608 vocabulary) and add a test that the scaffolded app's handler survives lowering.

**VERIFY**
```bash
cargo test -p flux-cli 2>&1 | tail -3
```
plus: `flux build` (or the crate's test harness) on a two-file fixture produces two distinct `.kt` files.

**COMMIT:** `fix(P2.23): per-source output filenames, deterministic entry point, hard error on unreadable source`

---

### T-405 — Generated-code compile gate in CI — Danger: MEDIUM

**Files:** new `.github/workflows/codegen-compile.yml`

Add a workflow that, on PRs touching `crates/flux-codegen-*` or `examples/`:
1. Runs `cargo build -p flux-cli`.
2. For each of `examples/counter`, `examples/todo`, `examples/router`: runs the CLI to emit Swift + Kotlin code.
3. Compiles the Swift output with `swiftc -typecheck` (macOS runner) and the Kotlin output with `kotlinc -script` typecheck or a minimal gradle project (ubuntu runner); if a toolchain is unavailable, the job must FAIL with `TOOLCHAIN MISSING: <name>` — never silently skip (that is the H31 anti-pattern).
4. Guards: `permissions: contents: read` + `concurrency` (per T-008 conventions).

**VERIFY:** `bash -n .github/workflows/codegen-compile.yml && echo SYNTAX-OK` (workflow correctness is proven on the next CI run).

**COMMIT:** `ci(codegen): compile generated Swift/Kotlin for stdlib examples (release gate for Phase 4)`

---

# PHASE 5 — PARITY HARNESS AND CONTRACT ENFORCEMENT

**Goal:** the harness would have caught everything in audit §4; stdlib↔kit drift cannot land silently.

---

### T-501 — Parity model must compare props [H24] — Danger: HIGH

**Files:** `crates/flux-parity/src/reduce.rs` (~105), `crates/flux-codegen-core/src/view_tree.rs` (~199–203), both parity recognizers (`grep -rln "recognizer" crates/flux-parity/src | head`)

The harness hardcodes `vec![]` for props — different labels/colors/text between dev and release (and between backends) stay green.

1. In `view_tree.rs`, thread the node's `props` into `ViewNode` (add the field; populate from the view tree the codegen builds).
2. In `reduce.rs`, replace the hardcoded `vec![]` with the node's real props.
3. Update both recognizers (dev-side and release-side) to surface the same prop subset (label/text/color/size/alignment) in canonical order.
4. Equivalence: props compare as maps — same keys, same rendered values.
5. Tests in `crates/flux-parity/tests/props_compare.rs`: dev `Button(text: "Save")` vs release emitting `text: "Saves"` must FAIL parity; identical props must PASS.

**VERIFY:** `cargo test -p flux-parity 2>&1 | tail -3`

**COMMIT:** `fix(H24): parity harness compares view props, not just structure`

---

### T-502 — Fix the four equivalence holes [P2.21, P2.22] — Danger: MEDIUM

**Files:** `crates/flux-parity/src/equivalence.rs` (~236–251, ~50–62, ~110–129), `crates/flux-parity/src/persistence.rs` (~187–197)

| Sub | Hole | Fix |
|---|---|---|
| T-502.1 | `branch_bag_equal` zip-compares swapped then/else | Match arms by their condition/pattern key, not by zip order (build a map keyed by the arm's discriminant, compare values). |
| T-502.2 | single-child containers elided | A dev `Row { Text }` == release `Column { Text }` passes: compare container kind too (keep an explicit `elide_single_child_containers = false` flag if legacy vectors need it). |
| T-502.3 | `norm_cond` treats every `/` as a comment opener | Properly lex: a comment starts at `//` only when outside a string literal; division survives. Add unit tests for `a / b` and `"http://x"`. |
| T-502.4 | persistence compares presence+type only | Compare the VALUES (deserialize both sides and compare the FluxValue). |

**VERIFY:** `cargo test -p flux-parity 2>&1 | tail -3` + one regression test per sub-item.

**COMMIT:** `fix(P2.21/22): parity equivalence compares branch order, containers, conditions, storage values`

---

### T-503 — Stdlib↔kit contract sweep [§6.8] — Danger: MEDIUM

**Files:** new `scripts/check-stdlib-props.py`, `.github/workflows/stdlib-contract.yml`

Nothing fails when stdlib declares a prop one or both kits never read, or when an adapter expects a different encoding than the IR produces.

1. Write `scripts/check-stdlib-props.py` (Python 3, stdlib only):
   - Input sources: `stdlib/*.flux` (parse `prop`/`param` declarations per component — the files are simple; a regex over `component X(...) { … }` parameter lists is acceptable, document its limits), Kotlin readers (`adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/PropsIndex.kt` + each `*Adapter.kt`'s `props.getX(PropsIndex.Y)` uses), Swift readers (`adapters/ui-swift/Sources/FluxUIKit/*.swift` prop reads).
   - Output: a table `component.prop → {declared, kotlin_reads, swift_reads, encodings_agree}`.
   - Exit 1 when: declared but read by NEITHER kit; declared and read by only ONE kit (list them); read by a kit but not declared.
2. The audit's known offenders to verify the script catches: `TextInput.keyboardType` (declared, read by neither — `PropsIndex.kt:75` defines the index but nothing reads it), `TextInput.ref`, `Text.overflow`, `Toggle` (registered in both kits but no `stdlib/toggle.flux`), `WebHost.src` (read via `src` by both kits but undeclared).
3. Wire the script into a new workflow (or append a step to an existing check workflow): `python3 scripts/check-stdlib-props.py || exit 1`.
4. Resolve the KNOWN offenders now: (a) delete `keyboardType`/`ref`/`overflow` from PropsIndex.kt or implement them in BOTH kits — deletion is the minimal change; (b) add `stdlib/toggle.flux` (component Toggle with `value: Bool` + `onChange` handler, matching both kits' registrations); (c) declare `WebHost.src: String` in `stdlib/web.flux`.

**VERIFY:** `python3 scripts/check-stdlib-props.py; echo "exit=$?"` → `exit=0` after the offender cleanup.

**COMMIT:** `fix(§6.8): stdlib↔kit prop contract sweep + resolve declared-but-unread props`

---

### T-504 — Land the native_kit_parity test claimed by the changelog [H29] — Danger: MEDIUM

**Files:** new `crates/flux-parity/tests/native_kit_parity.rs`

`CHANGELOG.md` (FLUX-078) claims this test gates adapter drift; the directory doesn't exist.

1. Write the test: for each pair `(component, props)` in a fixture table (Button, Text, TextInput, Image, Toggle, ScrollView, Column, Row, Stack), run the DEV-path recognizer and the RELEASE-path codegen→recognizer (both already exist in flux-parity; see T-501) and assert parity — including props now that T-501 threads them.
2. Run it in CI via the existing parity workflow or the new one from T-405.

**VERIFY:** `cargo test -p flux-parity native_kit 2>&1 | tail -3`

**COMMIT:** `fix(H29): land native_kit_parity.rs — the adapter-drift gate FLUX-078 claimed`

---

### T-505 — Wire fixtures + the three-decoder version gate [P2.40 / FLUX-083] — Danger: MEDIUM

**Files:** `fixtures/wire/` (populate), `runtimes/android/app/src/test/kotlin/dev/flux/app/WireFixtureContractTest.kt`, iOS `WireDecodeTests.swift`, new Rust test

`fixtures/wire/` contains only README.md; `unsupported-version.bin` doesn't exist; Kotlin silently skips (`assumeTrue`), Swift skips unless env-var set, and the Rust regeneration test doesn't exist — the FLUX-083 gate never runs.

1. Generate the fixtures from the RUST encoder (the source of truth). Add `crates/flux-ir-serde/examples/dump_fixtures.rs` (or a test) that writes, for a minimal fixed tree (one Text + one Button with a handler):
   - `fixtures/wire/init_v2.bin` — full Init frame.
   - `fixtures/wire/delta_v2.bin` — one prop-change Delta.
   - `fixtures/wire/unsupported_version.bin` — same frame with the version byte bumped to `0xFE`.
   Commit the three files.
2. Kotlin: remove the `assumeTrue` skip in `WireFixtureContractTest.kt`; it must decode `init_v2.bin`/`delta_v2.bin` and assert node counts + the rejected version path.
3. iOS: remove the env-var gate in `WireDecodeTests.swift`; same assertions.
4. Rust: `crates/flux-ir-serde/tests/fixtures_golden.rs` — decode the committed fixtures and assert they match a fresh encode (catches accidental encoder drift).

**VERIFY**
```bash
cargo test -p flux-ir-serde fixtures_golden 2>&1 | tail -3
cd runtimes/android && ./gradlew :app:testDebugUnitTest --console=plain 2>&1 | tail -4
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | grep WireDecode | tail -3
```

**COMMIT:** `fix(P2.40): commit wire fixtures generated by the encoder; three-decoder FLUX-083 gate runs`

---

### T-506 — Test-coverage symmetry ports [D24] — Danger: LOW

Port each side's missing tests. Table from the audit §4 D24 + §6.10:

| Port to | Test | Source reference |
|---|---|---|
| iOS | list-op ISA vectors | done in T-303 |
| iOS | registry completeness test | mirror Kotlin `AdapterRegistryTest.kt` |
| iOS | storage-doctch/corruption test | mirror Kotlin `StorageBackendTest.kt` |
| iOS | ScrollView adapter test | mirror Kotlin's ScrollView coverage |
| Kotlin | color/font decode tests | done in T-332 |
| Kotlin | Button/TextInput/Text/Props dedicated tests | mirror `adapters/ui-swift` dedicated test files |
| Both | UI tests in scheme | iOS `project.yml:62-104` defines `FluxAppUITests` but no scheme runs it — add the test target to the FluxApp scheme (`runtimes/ios/project.yml`) |

**VERIFY:** both suites green with the new tests.

**COMMIT:** `test(D24): port missing per-platform adapter/VM tests (coverage symmetry)`

---

### T-507 — Phase 5 exit gate

- `cargo test -p flux-parity` green including `native_kit_parity`.
- `python3 scripts/check-stdlib-props.py` exits 0.
- `fixtures/wire/` has the three .bin files and the three-decoder gate runs without skips.
- Prove the harness bites: temporarily revert ONE known-good prop in a fixture (e.g. change a Button label in the release path fixture), run the parity suite, confirm it FAILS, restore. Record `RED-ON-REVERT: yes` in PROGRESS.md.

# PHASE 6 — RELEASE HARDENING AND HYGIENE

**Goal:** shippable artifacts; no debug output in production paths; docs tell the truth.

---

### T-601 — CI remaining gates [H31, P2 CI list] — Danger: MEDIUM

| Sub | File(s) | Instruction |
|---|---|---|
| T-601.1 | `perf-harness.yml` (~59–79), `scripts/run-perf-harness.sh` (~53–79) | The on-device budget gate is a no-op: gradle failure swallowed (`\|\| true`), iOS under `set +e` → green with zero measurements. Remove `\|\| true`; wrap the iOS block in `set -e`; at the end of the script, verify the perf record file exists for EACH platform and `exit 1` with `PERF RECORD MISSING: <platform>` otherwise. |
| T-601.2 | `compat-matrix.yml` (~127) | `continue-on-error: true` while `release-gate.yml` `needs` it — remove `continue-on-error` or remove the `needs` edge; choose removal of `continue-on-error` (the gate must be real). |
| T-601.3 | `android-check.yml` (~82), `compat-matrix.yml` (~127) | `set -uo pipefail` without `-e`: add `-e`. |
| T-601.4 | hardcoded simulator names | Differ across workflows (`grep -rn "simulator\|destination" .github/workflows/*.yml | head`): pick one destination (e.g. `platform=iOS Simulator,name=iPhone 15`), extract to a top-level `env:` in each workflow, reference it. |
| T-601.5 | gradle wrapper | Add `distributionSha256Sum` to `gradle/wrapper/gradle-wrapper.properties` (obtain the checksum: `shasum -a 256 ~/.gradle/wrapper/dists/*/<zip>` after one download; if not obtainable offline, add a TODO comment and log SIDECAR). |
| T-601.6 | checkout@v4 vs v5 | Normalize ALL `actions/checkout` uses to one major (v4 — matches the most workflows). |
| T-601.7 | adr-numbering workflow | `adr-numbering.yml` triggers on paths that don't exist (`grep -n "paths" .github/workflows/adr-numbering.yml`) — fix the paths to the real ADR directory (`grep -rln "ADR-" docs 2>/dev/null \| head` or `find . -name "*adr*" -o -name "ADR*" | head`) or delete the workflow if ADRs live elsewhere. |

**VERIFY:** `for f in .github/workflows/*.yml; do bash -n "$f" || echo "BAD $f"; done` (yml syntax is not bash — instead verify with `python3 -c "import yaml,sys;[yaml.safe_load(open(f)) for f in __import__('glob').glob('.github/workflows/*.yml')]"` — adapt if PyYAML missing; at minimum grep-verify each change landed).

**COMMIT:** `ci(H31): perf gate fails on missing records; compat-matrix real; shell + pin hygiene`

---

### T-602 — CHANGELOG truth pass [H29 remainder, P3] — Danger: LOW

**Files:** `CHANGELOG.md`

1. Remove the duplicate `## [Unreleased]` header (grep for duplicates).
2. Add the FLUX-092 entry it lacks (one line: ForEach remove-wrong-row fixed on Android; residual identity bug tracked as C12 → fixed by T-330).
3. `website-check.yml` also claims ticket "FLUX-092" (`grep -n "FLUX-092" .github/workflows/website-check.yml`) — renumber that reference to a distinct ticket id or a plain description so ticket ids are unique.
4. Remove/repair references to nonexistent docs: `grep -rn "docs/release\|native_kit_parity\|crates/flux-parser/tests/stdlib.rs" CHANGELOG.md stdlib/README.md` — stdlib/README.md also claims 12 files when 29 exist: update the count to `find stdlib -name "*.flux" | wc -l` and cite the REAL gate (`scripts/parse-check.sh`).

**VERIFY:** `grep -c "## \[Unreleased\]" CHANGELOG.md` → `1`.

**COMMIT:** `docs: changelog truth pass (FLUX-092 entry, no phantom tests/files)`

---

### T-603 — Debug-output purge — Danger: LOW

| Sub | Target | Instruction |
|---|---|---|
| T-603.1 | `crates/flux-devtools-ui/src/views/component_tree.rs` (~60–292) | Remove the unconditional `eprintln!` calls or gate them behind a `debug_assertions`/feature flag. |
| T-603.2 | `crates/flux-devserver/src/pipeline.rs` (~743–746) | Delete the leftover per-node `tracing::debug!` loop in `build_init` (hot path). |
| T-603.3 | `crates/flux-ir-serde/src/frame.rs` (~1160–1164) | `intern_into` interns `""` for non-UTF-8: keep the fallback but add a `tracing::warn!` (once per call site, not per node) OR return an error — pick warn, log choice. |
| T-603.4 | Swift `print(` sweep | `grep -rn "print(" runtimes/ios/FluxHost/Sources/ --include="*.swift" \| grep -v "//"` — replace each with `Logger`/os_log or delete (test prints excluded). |
| T-603.5 | `FluxExecutor.swift` (~667) | Stop using `UserDefaults` as a log sink; move to the os_log Logger introduced in T-316.5. |
| T-603.6 | `FrameDeserializer.swift` (~89–93) | Delete the dead `dbg` closure. |

**VERIFY:** the greps above return no production hits.

**COMMIT:** `chore: debug-output purge across devtools, devserver, Swift host`

---

### T-604 — Remaining P2/P3 correctness items (batched sweep) — Danger: MEDIUM

Execute in order; each row is independent; VERIFY = touched crate's tests.

| ID | Finding | File(s) | Instruction |
|---|---|---|---|
| T-604.1 | P2.15 parser ty() fallback | `crates/flux-parser/src/parser.rs` (~1837–1847) | `ty()` accepts ANY token as a "primitive" type (`let x: 5` builds garbage). Restrict the fallback to the real primitive keyword set (the same list the lexer emits keywords for); otherwise return a syntax error `"expected type".` |
| T-604.2 | P2.16 parser recursion | `crates/flux-parser/src/parser.rs` (~1366, 1016, 1131, 1614) | Add a depth counter field to the parser; increment in expression/type/statement recursion entries; error `"expression nesting too deep"` past 512. Add `fuzz/fuzz_targets/parse_flux.rs` (mirror `decode_frame.rs`'s structure) + seed. |
| T-604.3 | P2.17 formatter | `crates/flux-parser/src/fmt/expr.rs` (~380–394, 270–294) | (a) Same-precedence right operands lose their parens (`a - (b - c)` → `a - b - c`): when printing a right operand of equal-or-lower precedence, keep the parens. (b) fmt never re-escapes `{`/`}` in interpolation strings — strings mutate on format: escape braces when emitting string literals. Tests: format(format(x)) == format(x) for both cases. |
| T-604.4 | P2.18 exhaust wildcard | `crates/flux-types/src/exhaust.rs` (~75–87) | An all-wildcard arm counts as catch-all even for a DIFFERENT ADT. Track the scrutinee's ADT; an arm of wildcard-only patterns matches only if its patterns' types unify with the scrutinee; else treat the match as non-exhaustive. |
| T-604.5 | P2.19 resolve 4 passes | `crates/flux-types/src/checker.rs` (~152–163) | Replace the fixed 4-pass substitution loop with fixpoint iteration (loop until the substitution stops changing, capped at 64 with an error). |
| T-604.6 | P2.20 field.rs silent var | `crates/flux-types/src/checker/field.rs` (~83–85) | Field access on unknown Named/Variant/List bases yields a fresh var (typos hide). Emit an error `"unknown field '<name>' on <base type>"` instead. |
| T-604.7 | P2.24 LSP expects | `crates/flux-lsp/src/lib.rs` (~132–488) | Replace the eight `.expect("mutex poisoned")` with proper error propagation (map to the LSP error type; a poisoned mutex is a `ServerInternalError` response, not a panic). |
| T-604.8 | P2.25 encoder desync | `crates/flux-ir-serde/src/wire/patch.rs` (~54–58), `wire/value.rs` (~35–39), `wire/child.rs` (~22–27) | Skipping unknown `#[non_exhaustive]` variants AFTER the parent count was written desyncs the stream. Error on unknown variants instead (they cannot be produced by this encoder; if decode-side tolerates them, keep decode tolerant). |
| T-604.9 | P3 Rust oracle | `crates/flux-vm-ref/src/vm.rs` | (a) EqF64: NaN == NaN must be FALSE (IEEE). (b) `truthy()` accepts any Int — restrict truthiness to Bool/Null semantics per the spec's `if` contract (error on Int scrutinee). (c) StrConcat overflow wraps — checked add with a VmError. (d) AWAIT ignores `result_reg` — honor it (deposit into the named register; compiler currently always passes 0, keep behavior compatible). |
| T-604.10 | P3 frame id collision | `crates/flux-ir-serde/src/telemetry.rs` (~34) vs `resume.rs` (~27) | `FRAME_HOST_ANNOUNCE` and `FRAME_AWAIT_SUSPEND` are both 0x12. Assign AWAIT_SUSPEND a fresh byte (0x20 or the next free slot; check `grep -rn "0x12\|0x13" crates/flux-ir-serde/src/*/` for the free list), update the decoder match arms + any tests. |
| T-604.11 | P3 span panic | `crates/flux-syntax/src/ids/span.rs` (~91–100) | `SourceExcerpt::from_span` can slice mid-UTF-8 and panic: use char-boundary-safe slicing (`floor_char_boundary`-style manual scan). |
| T-604.12 | P3 perf cleanups | `crates/flux-ir-serde/src/wire/core.rs` (~160–188), `crates/flux-devserver/src/pipeline/tree.rs`, `crates/flux-differ/src/diff/algorithm.rs` (~159–189) | (a) `validate_bytecode` pass-2 O(n²) → index map. (b) `tree.rs` `Vec::contains` dedup → `HashSet`; fix the comment that says BFS but implements DFS (or implement BFS — comment must match code). (c) `reattach_pairs` quadratic → hash-join on candidate keys. |
| T-604.13 | P3 multiset XOR | `crates/flux-ir-serde/src/encode.rs` (~97–108), `crates/flux-ir/src/arena/content_address.rs` (~150–157) | XOR-fold multiset hashes cancel duplicates (`{a,a,b}` == `{b}`): replace XOR with an order-insensitive sum (wrapping add) or per-element combine-then-sum. Update any golden hashes. |
| T-604.14 | P3 fdiv −0.0 | `FluxBytecodeVM.swift` (~1422–1428) | Swift `fdiv` mishandles −0.0 sign vs the oracle: match `vm.rs`'s division semantics exactly (`x / y` with IEEE sign; special-case 0/± for sign parity). Add a vector to `tests/isa-vectors/typed_arith.json`. |
| T-604.15 | P3 misc Swift | `InternString.swift` (~82–93), `FluxBytecodeVM.swift` (~531, 1065) | (a) `internStringFrameBytes` sends protocol v1 + `UInt16` trap on >64KB — send v2 + checked length (error, not trap). (b) Replace boxed `as!` casts with guarded `as?` + error. |
| T-604.16 | P3 event-verb vocabulary | parser/IR + both kits + stdlib | The audit lists inconsistent verbs (`onPress`/`onChange`/`onValueChange`/`onGesture`, `onClick` dropped). Do NOT rename everything (wire-visible); instead: document the canonical set in `docs/appendix-f-parity.md`, add alias acceptance in `collect_handler` for the documented aliases mapping to one canonical signal, and file the canonical table as SIDECAR if aliases are unacceptable. |
| T-604.17 | P3 naming collisions | `stdlib/router.flux` (~19) | `Router.initialRouteName` dead + `capability Router`/`compo Router` collision: remove the dead prop; rename the capability or component (pick `Router` for the component, `RouterNav` for the capability — update `capabilities.flux`, both registries, and `CAPABILITY_IDL`). |
| T-604.18 | P3 grammar check | `syntaxes/flux.tmLanguage.json` (~44) | `check-grammar.mjs` fails: add `record` to the keyword list in the committed tmLanguage grammar. |
| T-604.19 | P3 fuzz targets | `fuzz/fuzz_targets/` | Add targets for: `telemetry_frame`, `debug_command`, `await_suspend_resume`, `dispatch_report`, `host_announce`, `intern_string`, `decode_value_blob`, `validate_bytecode` (each mirrors `decode_frame.rs`'s harness: `cargo fuzz` structure already present). Seeds: 16-byte zeros per target (T-006 pattern). |
| T-604.20 | P3 dead code | `FluxBytecodeVM.swift` `assertCanonicalStringId` (zero production callers), `fluxTrace` (never called) | Delete both, or wire `assertCanonicalStringId` into the string-interning paths where the canonical-id invariant should hold. Prefer deletion. |

**VERIFY (after the sweep):** `cargo test --workspace 2>&1 | tail -6`, both platform suites green.

**COMMIT:** one per row: `chore(P3): <one-liner>`.

---

### T-605 — Final release rehearsal — Danger: MEDIUM

1. `cargo build --release --workspace` — clean.
2. `cd runtimes/android && ./gradlew :app:assembleRelease --console=plain` — produces a minified, signed (debug-signing acceptable in CI) APK/AAR per T-335.7.
3. `cd runtimes/ios/FluxHost && xcodebuild build -scheme FluxHost -configuration Release -destination 'platform=macOS'` — clean (H11 done).
4. `bash scripts/release-gate/check-contract-freeze.sh` — PASS.
5. `bash scripts/ci-size-gate.sh` (in `--all` mode if supported) — zero violations or only whitelisted ones.
6. `bash scripts/parse-check.sh` — all stdlib files parse.
7. `python3 scripts/check-stdlib-props.py` — exit 0.
8. Full test suites: `cargo test --workspace`, `./gradlew :host:testDebugUnitTest`, `xcodebuild test -scheme FluxHost`.
9. Record every command + result in PROGRESS.md under `## Release rehearsal`.

---

## APPENDIX A — TASK INDEX (EXECUTION ORDER)

| Phase | Tasks | Finding refs | Theme |
|---|---|---|---|
| 0 | T-001…T-009 | C13, H13, C14, H28, H30 | CI/tooling unblock |
| 1 | T-101…T-111 | C1–C6, H18–H20, P2.10–P2.14 | Compiler correctness |
| 2 | T-201…T-222 | C7–C9, H13–H17, P2.1–P2.9 | Wire/differ/devserver |
| 3 | T-301…T-316 (Swift), T-330…T-336 (Kotlin/cross) | C10–C12, H1–H12, D1–D24, P2.26–P2.40 | Host runtimes + parity drift |
| 4 | T-401…T-405 | H21, H22, §5.1–5.11, P2.23 | Release codegen |
| 5 | T-501…T-507 | H24, H29, P2.21/22/40, §6.8, D24 | Parity + contracts |
| 6 | T-601…T-605 | H26, H31, P2.x remainder, P3 | Hardening + hygiene |

Dependencies to respect: T-101→T-102; T-201→T-205 (timeout half); T-301→T-302; T-308 after T-307 (same file, additive); T-330 before T-331; T-331 supersedes T-334.3 and T-335.3; T-312 supersedes T-334.6; T-501 before T-504; T-604 may interleave with Phase 5 if suites block.

## APPENDIX B — COMMAND CHEAT SHEET

```bash
# Rust
cargo build --workspace && cargo test --workspace
cargo test -p <crate>                     # single crate
cargo build -p <crate>                    # fast compile loop

# Kotlin (host module tests; app module tests with :app:)
cd runtimes/android && ./gradlew :host:testDebugUnitTest --console=plain
cd runtimes/android && ./gradlew :app:assembleRelease --console=plain

# Swift
cd runtimes/ios/FluxHost && xcodebuild test -scheme FluxHost -destination 'platform=macOS' 2>&1 | tail -5
cd runtimes/ios/FluxHost && xcodebuild build -scheme FluxHost -configuration Release -destination 'platform=macOS' 2>&1 | tail -3
cd adapters/ui-swift && xcodebuild test -scheme FluxUIKit -destination 'platform=macOS' 2>&1 | tail -5

# Gates & scripts
bash scripts/release-gate/check-contract-freeze.sh
bash scripts/ci-size-gate.sh
bash scripts/parse-check.sh
python3 scripts/check-stdlib-props.py
```

## APPENDIX C — PROGRESS.md TEMPLATE

```markdown
# Fix-Playbook Progress

## Baseline (R6)
- cargo build: <ok/fail>
- cargo test: <pass counts / first failing crate>
- gradlew :host:testDebugUnitTest: <ok/fail/toolchain missing>
- xcodebuild FluxHost: <ok/fail/toolchain missing>

## Task log
| Task | Status | Notes |
|---|---|---|
| T-001 | DONE | commit <sha> |
| T-002 | BLOCKED | OLD block not found at gradle/wrapper/ |
| ... | | |

## Sidecars (R4)
- <file:line>: <issue noticed, not fixed>

## Release rehearsal (T-605)
- <command>: <result>
```

## APPENDIX D — DEFINITION OF DONE ("PRODUCTION READY")

The codebase may be called production-ready when ALL of the following hold:

1. **Phases 0–3 complete**: every task DONE or explicitly logged BLOCKED/PARTIAL with a reason; no FAILED tasks left unreverted.
2. **Zero silent miscompiles**: the Phase 1 conformance vectors run on all three runtimes (`tests/isa-vectors/` consumed by Rust oracle + Kotlin + Swift).
3. **Hot reload cannot corrupt**: differ golden tests (multi-insert order, root insert, handler patches) green; token-gate rejection test green.
4. **Capabilities work on both platforms**: Http/Persist execute on iOS through CALL_CAP under a real permission checker; no `DispatchSemaphore` on the main actor.
5. **No leaks / identity churn**: leak test and double-mutation test green; `built`/`nodes` bounded.
6. **One identity scheme**: `tests/isa-vectors/foreach_ids.json` vectors identical on both hosts; splice keys are row identity.
7. **Generated code compiles**: `codegen-compile.yml` green for counter/todo/router on both toolchains.
8. **The harness bites**: parity suite RED-ON-REVERT proven (T-507); `native_kit_parity` green; stdlib sweep exit 0.
9. **Release rehearsal (T-605)**: every command green; artifacts minified + signed; telemetry debug-only.
10. **PROGRESS.md** contains no open NEEDS DECISION rows.

---

*End of playbook. Work top-to-bottom, verify everything, commit each task, and never touch what a task does not name.*

