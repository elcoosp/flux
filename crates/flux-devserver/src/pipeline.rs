//! The compile pipeline: source → parse → type check → lower → diff → frame.
//!
//! [`Pipeline`] owns the last-good [`LoweredIr`] for the project plus the
//! server-side [`DependencyIndex`] derived from each tree's `signal_deps`
//! (ADR-0027 Phase 2, server half). Compiling a source snapshot either advances
//! that state and yields the frame to ship, or returns a [`Diagnostic`] — in
//! which case the previous good tree is retained and no `Delta` is produced
//! (spec §D.12.3).

/// The result of compiling all source snapshots at once: the merged wire arena,
/// the handler closure table, and the per-file `(path, LoweredIr, Ast)` bundles
/// used by the release codegen path.
#[derive(Debug)]
struct TreeCompilation {
    /// Merged arena for the wire frame.
    arena: IRArena,
    /// Handler closures across all files.
    ///
    /// This includes each node's compiled prop thunk (ADR-0027 T14 / ADR-0043):
    /// a thunk is a closure like any other, and shipping it in the frame's
    /// shared handler blob is what lets the host resolve a node's `prop_thunk`
    /// `ClosureRef` to real bytecode and evaluate it on dirty reconciliation.
    closures: Vec<flux_ir::ClosureIR>,
    /// Initial state-signal values, gathered from every source's lowered IR.
    state_seed: Vec<(flux_syntax::SignalId, flux_syntax::Value)>,
    /// Per-source lowered programs for the codegen path.
    sources: Vec<(PathBuf, LoweredIr, Ast)>,
    /// Per-node prop thunks, merged across files, retained so the last-good
    /// tree can be re-serialised into an `Init` frame for a reconnecting host.
    prop_thunks: std::collections::HashMap<flux_syntax::NodeId, flux_ir::ClosureIR>,
    /// Component-id → name pairs, merged across files, shipped in the `Init`
    /// frame so a host resolves each node's adapter from its `ComponentId`.
    component_names: Vec<(flux_syntax::ComponentId, String)>,
    /// Generic instantiations merged across files (roadmap Phase 1), so the
    /// release backends emit one specialised native type per instantiation.
    monomorphizations: Vec<flux_ir::Monomorphization>,
}

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use flux_ir::{IRArena, LoweredIr, lower};
use flux_ir_serde::{Frame, InitFrame, NodeSignalMeta};
use flux_parser::Ast;
use flux_perf_harness::{LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario};
use flux_syntax::{FileId, Patch, SignalId, SourceExcerpt, StringId, Value};

use crate::dispatch::{DependencyIndex, DispatchReport, NodeSignalDeps, emit_minimal_updates};
use crate::error::Diagnostic;
use flux_types::{CompileError, FluxError, ModuleLoader};
use host_strings::HostStrings;
use tree::{display_path, flatten_extra_nodes, merge_arenas, root_node};

pub(crate) mod host_strings;
pub(crate) mod tree;

/// The outcome of compiling one source snapshot.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Compiled {
    /// The first successful compile: ship a full `Init`.
    Init(Vec<u8>),
    /// A successful recompile that changed the tree: ship a `Delta`.
    Delta(Vec<u8>),
    /// A successful recompile that changed nothing: ship nothing.
    Unchanged,
}

/// Per-phase timings recorded when `--profile` is enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhaseTimings {
    /// Time spent in [`flux_parser::parse`].
    pub parse: std::time::Duration,
    /// Time spent in [`flux_types::type_check`].
    pub type_check: std::time::Duration,
    /// Time spent in [`flux_ir::lower()`].
    pub lower: std::time::Duration,
    /// Time spent in [`flux_differ::diff`].
    pub diff: std::time::Duration,
    /// Time spent serializing the frame.
    pub serialize: std::time::Duration,
}

/// The incremental compile pipeline for one project root.
///
/// The pipeline is single-threaded state; the server guards it with a mutex and
/// drives it from the file-watch task.
#[derive(Debug)]
pub struct Pipeline {
    root: PathBuf,
    /// Source snapshot per watched file, keyed by its assigned [`FileId`].
    sources: BTreeMap<FileId, (PathBuf, String)>,
    /// Stable `path → FileId` assignment (dense, insertion-ordered).
    file_ids: BTreeMap<PathBuf, FileId>,
    /// The last successfully lowered program, or `None` before the first
    /// successful compile.
    last_good: Option<LoweredIr>,
    /// Per-source lowered programs from the last good compile, for the release
    /// codegen path. Empty before the first successful compile.
    last_sources: Vec<(PathBuf, LoweredIr, Ast)>,
    /// Monotonic wire sequence number.
    seq: u32,
    profile: bool,
    /// Timings of the most recent compile.
    timings: PhaseTimings,
    /// Reverse `SignalId → {NodeId}` index used to scope dispatch patches to the
    /// nodes that actually read a written signal (ADR-0027 Phase 2). Inactive
    /// until the lowered tree carries `signal_deps` (injected via
    /// [`set_signal_deps`](Self::set_signal_deps) until FA-IRWIRE lands T13).
    index: DependencyIndex,
    /// Per-node `signal_deps` for the last good tree. `None` before the first
    /// compile or when no dependency data is available, which leaves `index`
    /// inactive and degrades the server to coarse frames.
    signal_deps: Option<Vec<NodeSignalDeps>>,
    /// Strings interned on behalf of a connected host (brittleness 4a).
    ///
    /// Kept separate from the compiler's arena table so a host request can never
    /// perturb the ids the tree itself was serialised with. Ids reported to the
    /// host are dense within the module's own reserved region.
    host_strings: HostStrings,
    /// Reusable scratch buffer for frame encoding (OPT-B). Held across compiles
    /// so the per-edit `Delta`/`Init` hot path performs no fresh allocation
    /// after warm-up — the encoder clears and refills it in place.
    scratch: Vec<u8>,
    /// Per-file lowered result cache keyed by [`FileId`], storing the source
    /// snapshot it was produced from so an unchanged file is **not** re-parsed,
    /// re-type-checked, or re-lowered on a recompile (FLUX-074, item B:
    /// incremental lowering — bounded work to the changed subtree).
    file_cache: BTreeMap<FileId, (String, LoweredIr, Ast)>,
    /// Number of files actually (re-)lowered in the most recent
    /// [`compile_tree`](Self::compile_tree) — incremented only when a file's
    /// source differs from its cache entry. Used as the bounded-work metric for
    /// FLUX-074 item B.
    lower_count: u64,
}

impl Pipeline {
    /// Creates an empty pipeline rooted at `root`.
    #[must_use]
    pub fn new(root: impl AsRef<Path>, profile: bool) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            sources: BTreeMap::new(),
            file_ids: BTreeMap::new(),
            last_good: None,
            last_sources: Vec::new(),
            seq: 0,
            profile,
            timings: PhaseTimings::default(),
            index: DependencyIndex::default(),
            signal_deps: None,
            host_strings: HostStrings::default(),
            scratch: Vec::new(),
            file_cache: BTreeMap::new(),
            lower_count: 0,
        }
    }

    /// Per-phase timings of the most recent compile.
    #[must_use]
    pub fn timings(&self) -> PhaseTimings {
        self.timings
    }

    /// Builds the render-perf [`MetricRecord`]s captured during the most recent
    /// compile so the dev server can broadcast them to DevTools as `PerfRecord`
    /// telemetry (FLUX-059 / PRD-J).
    ///
    /// The records describe the server-side half of the spec's `Save → pixels`
    /// budget (§3.10): the full pipeline (parse + type check + lower + diff +
    /// serialize) is reported as `Scenario::LoopbackE2e` / `MetricKind::SaveToPhoton`,
    /// and the patch-serialization leg alone as `MetricKind::PatchRoundTrip`.
    /// `tree_size` is the lowered node count (`lower_count`), giving the flamegraph
    /// the same tree-size axis the harness uses. Returns empty until a compile has
    /// recorded timings (i.e. before the first successful compile).
    #[must_use]
    pub fn perf_records(&self) -> Vec<MetricRecord> {
        let t = self.timings;
        if t == PhaseTimings::default() {
            return Vec::new();
        }
        let tree_size = self.lower_count;
        let total_ms =
            (t.parse + t.type_check + t.lower + t.diff + t.serialize).as_secs_f64() * 1000.0;
        let serialize_ms = t.serialize.as_secs_f64() * 1000.0;
        vec![
            MetricRecord::new(
                Scenario::LoopbackE2e,
                MetricKind::SaveToPhoton,
                tree_size,
                vec![MetricSample::latency(LatencyMs::from_raw(total_ms))],
            ),
            MetricRecord::new(
                Scenario::LoopbackE2e,
                MetricKind::PatchRoundTrip,
                tree_size,
                vec![MetricSample::latency(LatencyMs::from_raw(serialize_ms))],
            ),
        ]
    }

    /// Interns `text` on behalf of a host and returns its canonical
    /// [`StringId`] (brittleness 4a).
    ///
    /// A string already present in the compiled tree's own table resolves to
    /// that arena id, so the host and the wire tree agree. Anything else is
    /// interned into the server's own reserved host-string region. The returned
    /// id is always below
    /// [`flux_ir_serde::STRING_ID_CANONICAL_CEILING`], which is what lets a host
    /// drop its synthetic-hash fallback; interning the same text twice returns
    /// the same id.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_devserver::Pipeline;
    ///
    /// let mut pipeline = Pipeline::new(".", false);
    /// let first = pipeline.intern_string("tap");
    /// assert_eq!(pipeline.intern_string("tap"), first);
    /// assert!(first < flux_ir_serde::STRING_ID_CANONICAL_CEILING);
    /// ```
    pub fn intern_string(&mut self, text: &str) -> StringId {
        if let Some(last) = &self.last_good
            && let Some(id) = last.arena.string_table().lookup(text)
        {
            return id;
        }
        self.host_strings.intern(text)
    }

    /// Resolves a host-interned string previously assigned by
    /// [`intern_string`](Self::intern_string), or `None` when `id` was never
    /// handed out from the host-string region.
    #[must_use]
    pub fn resolve_host_string(&self, id: StringId) -> Option<&str> {
        self.host_strings.resolve(id)
    }

    /// Whether a good tree has been compiled at least once.
    #[must_use]
    pub fn has_tree(&self) -> bool {
        self.last_good.is_some()
    }

    /// The number of files actually (re-)lowered in the most recent
    /// [`compile`](Self::compile) — incremented only when a file's source
    /// differs from its cached entry (FLUX-074, item B: incremental lowering).
    /// Files whose source is unchanged across a recompile are reused from the
    /// cache and do not count.
    #[must_use]
    pub fn lower_count(&self) -> u64 {
        self.lower_count
    }

    /// The capability methods the last-good tree actually requires.
    ///
    /// Enumerates every `CALL_CAP` in the compiled handler and prop-thunk
    /// bytecode (Appendix E §E.1), returning each distinct `(cap_id,
    /// method_id)` pair it needs from the host. The dev server checks this
    /// against the capabilities a connecting host advertises in its `Hello`,
    /// so a host missing a required camera/storage/router method fails the
    /// handshake with an actionable `Error` frame instead of crashing at the
    /// first `CALL_CAP` (spec §D.12.1 / §24.4).
    ///
    /// Returns an empty vector before the first successful compile.
    #[must_use]
    pub fn required_capabilities(&self) -> Vec<(u32, u16)> {
        use flux_syntax::opcode::Opcode;

        let Some(ir) = &self.last_good else {
            return Vec::new();
        };
        let mut seen: Vec<(u32, u16)> = Vec::new();
        let mut visit = |bytecode: &[u8]| {
            let mut ip = 0usize;
            while ip < bytecode.len() {
                let Some(op) = Opcode::from_byte(bytecode[ip]) else {
                    break;
                };
                let n = op.operand_len() as usize;
                let start = ip + 1;
                let end = start + n;
                if op == Opcode::CallCap && end <= bytecode.len() {
                    // Layout: result_reg(u8) | cap_id(u32 LE) | method_id(u16 LE) | args_reg(u8).
                    let cap_id = u32::from_le_bytes([
                        bytecode[start + 1],
                        bytecode[start + 2],
                        bytecode[start + 3],
                        bytecode[start + 4],
                    ]);
                    let method_id = u16::from_le_bytes([bytecode[start + 5], bytecode[start + 6]]);
                    if !seen.contains(&(cap_id, method_id)) {
                        seen.push((cap_id, method_id));
                    }
                }
                if end > bytecode.len() {
                    break;
                }
                ip = end;
            }
        };
        for closure in ir.closures.values() {
            visit(&closure.bytecode);
        }
        for thunk in ir.prop_thunks.values() {
            visit(&thunk.bytecode);
        }
        seen.sort_unstable();
        seen
    }

    /// One lowered program per source file from the last good compile, for the
    /// release codegen path (`flux build`).
    ///
    /// Each entry pairs a source file's path with its lowered IR and parsed AST,
    /// exactly as a downstream codegen crate needs it (see the
    /// `codegen-input-contract` ADR, which settles the `(LoweredIr, Ast)`
    /// signature). Returns an empty vector before the first successful compile.
    /// Cloning is acceptable: MLP trees are small.
    #[must_use]
    pub fn compiled_sources(&self) -> Vec<(PathBuf, LoweredIr, Ast)> {
        self.last_sources
            .iter()
            .map(|(path, ir, ast)| (path.clone(), ir.clone(), ast.clone()))
            .collect()
    }

    /// The generic instantiations the last good compile specialised, merged
    /// across every source file (roadmap Phase 1).
    ///
    /// A generic component instantiated in two different files (`Counter[Int]`
    /// in one, `Counter[Float]` in another) yields one entry per distinct
    /// instantiation, deduplicated — the release backends emit one native type
    /// per entry, so a duplicate would emit the same type twice and a missing
    /// entry would erase the type argument. Empty before the first successful
    /// compile and for programs with no generic call sites.
    #[must_use]
    pub fn monomorphizations(&self) -> Vec<flux_ir::Monomorphization> {
        self.last_good
            .as_ref()
            .map(|ir| ir.monomorphizations.clone())
            .unwrap_or_default()
    }

    /// Builds the DevTools [`SourceMap`](crate::SourceMap) from the last-good
    /// lowered IR so the
    /// debug bridge can enrich telemetry with `.flux` source spans (Phase 3).
    ///
    /// Prefers the retained last-good tree; falls back to the first
    /// compiled source; returns an empty map when nothing has compiled yet.
    #[must_use]
    pub fn devtools_source_map(&self) -> crate::debug_bridge::SourceMap {
        if let Some(ir) = &self.last_good {
            return crate::debug_bridge::SourceMap::from_lowered(ir);
        }
        self.compiled_sources()
            .into_iter()
            .next()
            .map(|(_, ir, _)| crate::debug_bridge::SourceMap::from_lowered(&ir))
            .unwrap_or_default()
    }

    /// Injects the per-node `signal_deps` for the current tree (ADR-0027 Phase 2).
    ///
    /// The lowered IR (FA-IRWIRE, T13) will eventually carry `signal_deps` on
    /// every node; until then the server-side index is fed by this method, which
    /// the file-watch path and the integration harness both call. Passing `None`
    /// clears the dependency data and forces the server back to coarse-frame
    /// behaviour (the degradation path). The index is rebuilt immediately so a
    /// subsequent [`handle_dispatch_report`](Self::handle_dispatch_report) sees
    /// the new mapping.
    pub fn set_signal_deps(&mut self, deps: Option<Vec<NodeSignalDeps>>) {
        self.signal_deps = deps;
        self.rebuild_index();
    }

    /// Rebuilds the reverse index from the lowered tree's real `signal_deps`
    /// (FA-IRWIRE T13), or from the injected set when one was supplied via
    /// [`set_signal_deps`](Self::set_signal_deps) (legacy / test hook). When no
    /// dependency data is available the index is cleared and the server
    /// degrades to coarse-frame behaviour.
    fn rebuild_index(&mut self) {
        // Prefer the injected set (test harness / file-watch path); otherwise
        // derive from the retained lowered tree's per-node `signal_deps_of`.
        if let Some(deps) = &self.signal_deps {
            self.index.rebuild(deps);
        } else if let Some(last) = &self.last_good {
            let deps: Vec<NodeSignalDeps> = last
                .arena
                .all_ids()
                .filter_map(|id| {
                    let deps = last.arena.signal_deps_of(id);
                    if deps.is_empty() {
                        None
                    } else {
                        Some(NodeSignalDeps {
                            id,
                            signal_deps: deps.to_vec(),
                        })
                    }
                })
                .collect();
            self.index.rebuild(&deps);
        } else {
            self.index = DependencyIndex::default();
            return;
        }
        tracing::debug!(
            edges = self.index.edge_count(),
            active = self.index.is_active(),
            "rebuilt signal→node dependency index"
        );
    }

    /// Handles a host dispatch report, returning the minimal `Delta` frame to
    /// ship, or `None` when the report should not produce a server patch.
    ///
    /// Returns `None` in two cases, both of which mean "the caller must not ship
    /// a minimal patch":
    /// * the index is inactive (no `signal_deps` in the tree) — degrade to the
    ///   coarse-frame path;
    /// * the written signal has no dependents — zero patches would be sent, and
    ///   the ADR-0027 `noop_dispatch` budget requires *nothing* to leave the
    ///   server.
    ///
    /// The released patch set is addressed only to `dependents[written]`; nodes
    /// outside that set receive nothing (the bounded-by-`|dependents[S]|`
    /// guarantee).
    #[must_use]
    pub fn handle_dispatch_report(&mut self, report: DispatchReport) -> Option<Vec<u8>> {
        let last = self.last_good.as_ref()?;
        let arena = &last.arena;
        let written = report.written;
        match emit_minimal_updates(written, arena, &self.index) {
            Ok(patches) if patches.is_empty() => None,
            Ok(patches) => {
                let frame = self.build_dispatch_delta(&patches);
                tracing::debug!(
                    handler = ?report.handler_id,
                    written = written,
                    patches = patches.len(),
                    "emitted minimal-patch delta for dispatch"
                );
                Some(frame)
            }
            // Inactive index (no signal_deps yet) → fall back to coarse frame.
            Err(_) => None,
        }
    }

    /// Builds a `Delta` frame carrying `patches`, addressed from the report.
    fn build_dispatch_delta(&mut self, patches: &[Patch]) -> Vec<u8> {
        let strings: Vec<(flux_syntax::StringId, String)> = self
            .last_good
            .as_ref()
            .map(|last| {
                last.arena
                    .string_table()
                    .iter()
                    .map(|(id, text)| (id, text.to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let closures = self
            .last_good
            .as_ref()
            .map(|last| last.closures.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        self.seq = self.seq.wrapping_add(1);
        Frame::delta(
            self.seq,
            flux_ir_serde::FLAG_HAS_STRING_DELTA,
            patches,
            &strings,
            &closures,
            &[],
        )
        .to_bytes()
    }

    /// Assigns (or reuses) the dense [`FileId`] for `path`.
    fn file_id_for(&mut self, path: &Path) -> FileId {
        if let Some(id) = self.file_ids.get(path) {
            return *id;
        }
        let id = FileId::from(self.file_ids.len() as u32);
        self.file_ids.insert(path.to_path_buf(), id);
        id
    }

    /// Records a source snapshot for `path`, replacing any previous snapshot.
    pub fn set_source(&mut self, path: &Path, source: String) {
        let id = self.file_id_for(path);
        self.sources.insert(id, (path.to_path_buf(), source));
    }

    /// The `(FileId, path)` source map shipped in the `Init` frame.
    fn source_map(&self) -> Vec<(FileId, String)> {
        self.sources
            .iter()
            .map(|(id, (path, _))| (*id, display_path(&self.root, path)))
            .collect()
    }

    /// Compiles the current source snapshots.
    ///
    /// On success the internal last-good tree advances and the frame to ship is
    /// returned. On failure the previous tree is retained and the caller ships
    /// an `Error` frame built from the returned [`Diagnostic`].
    ///
    /// # Errors
    ///
    /// Returns a [`Diagnostic`] when parsing, type checking or lowering fails.
    pub fn compile(&mut self) -> Result<Compiled, Diagnostic> {
        let TreeCompilation {
            arena,
            closures,
            state_seed,
            sources,
            prop_thunks,
            component_names,
            monomorphizations,
        } = self.compile_tree()?;
        let started = Instant::now();
        let outcome = match self.last_good.as_ref() {
            None => {
                let frame = self.build_init(&arena, &closures, &state_seed, &component_names);
                self.timings.serialize = started.elapsed();
                Compiled::Init(frame)
            }
            Some(previous) => {
                let diff_started = Instant::now();
                let patches = flux_differ::diff(&previous.arena, &arena);
                self.timings.diff = diff_started.elapsed();
                if patches.is_empty() {
                    Compiled::Unchanged
                } else {
                    let frame = self.build_delta(&arena, &patches, &closures);
                    self.timings.serialize = started.elapsed();
                    Compiled::Delta(frame)
                }
            }
        };
        self.last_good = Some(LoweredIr {
            arena,
            closures: closures.iter().map(|c| (c.id, c.clone())).collect(),
            prop_thunks: prop_thunks.clone(),
            component_names: component_names.clone(),
            monomorphizations: monomorphizations.clone(),
            state_seed: state_seed.clone(),
            instances: flux_ir::InstanceRegistry::new(),
        });
        self.last_sources = sources;
        // The dependency index is a pure function of the tree's `signal_deps`. The
        // real source will be the lowered nodes (FA-IRWIRE T13); until then the
        // injected set (see `set_signal_deps`) describes this same tree, so
        // re-derive the index whenever the tree advances.
        self.rebuild_index();
        if self.profile {
            let t = self.timings;
            tracing::info!(
                parse_us = t.parse.as_micros(),
                type_check_us = t.type_check.as_micros(),
                lower_us = t.lower.as_micros(),
                diff_us = t.diff.as_micros(),
                serialize_us = t.serialize.as_micros(),
                "pipeline phase timings"
            );
        }
        Ok(outcome)
    }

    /// Runs parse → type check → lower over every source snapshot, merging the
    /// per-file arenas into a single tree for the wire path and retaining each
    /// file's `(path, LoweredIr, Ast)` for the codegen path.
    fn compile_tree(&mut self) -> Result<TreeCompilation, Diagnostic> {
        self.lower_count = 0;
        let snapshots: Vec<(FileId, PathBuf, String)> = self
            .sources
            .iter()
            .map(|(id, (path, src))| (*id, path.clone(), src.clone()))
            .collect();
        let mut merged: Option<IRArena> = None;
        let mut closures: Vec<flux_ir::ClosureIR> = Vec::new();
        let mut state_seed: Vec<(SignalId, Value)> = Vec::new();
        let mut sources: Vec<(PathBuf, LoweredIr, Ast)> = Vec::with_capacity(snapshots.len());
        let mut prop_thunks: std::collections::HashMap<flux_syntax::NodeId, flux_ir::ClosureIR> =
            std::collections::HashMap::new();
        let mut component_names: Vec<(flux_syntax::ComponentId, String)> = Vec::new();
        let mut monomorphizations: Vec<flux_ir::Monomorphization> = Vec::new();
        for (file_id, path, source) in snapshots {
            let display = display_path(&self.root, &path);
            // Incremental lowering (FLUX-074, item B): reuse the previously
            // lowered result when a file's source is unchanged, so a save that
            // touches one file does not re-parse / re-type-check / re-lower the
            // others. `lower_count` records only the files actually re-lowered.
            let (lowered, ast) =
                if let Some((cached_src, cached_ir, cached_ast)) = self.file_cache.get(&file_id) {
                    if cached_src == &source {
                        (cached_ir.clone(), cached_ast.clone())
                    } else {
                        self.lower_count += 1;
                        let produced = self.compile_one(&source, file_id, &display)?;
                        self.file_cache.insert(
                            file_id,
                            (source.clone(), produced.0.clone(), produced.1.clone()),
                        );
                        produced
                    }
                } else {
                    self.lower_count += 1;
                    let produced = self.compile_one(&source, file_id, &display)?;
                    self.file_cache.insert(
                        file_id,
                        (source.clone(), produced.0.clone(), produced.1.clone()),
                    );
                    produced
                };
            closures.extend(lowered.closures.values().cloned());
            closures.extend(lowered.prop_thunks.values().cloned());
            prop_thunks.extend(
                lowered
                    .prop_thunks
                    .iter()
                    .map(|(id, thunk)| (*id, thunk.clone())),
            );
            component_names.extend(lowered.component_names.iter().cloned());
            for mono in &lowered.monomorphizations {
                if !monomorphizations.contains(mono) {
                    monomorphizations.push(mono.clone());
                }
            }
            state_seed.extend(lowered.state_seed.iter().cloned());
            sources.push((path.clone(), lowered.clone(), ast));
            merged = Some(match merged {
                None => lowered.arena,
                // Multi-file projects merge by packing the later file's nodes
                // into the first arena; node IDs are content-derived so the
                // merge cannot collide across files with distinct spans.
                Some(base) => merge_arenas(base, &lowered.arena),
            });
        }
        // Populate the arena's closure table. `lowered.closures` (and the
        // prop_thunks folded in below) are the authoritative closure bodies, but
        // `IRArena::closure()` reads from the arena's own `closures` map, which
        // lowering never fills via `add_closure`. Without this, the differ's
        // `handlers_equal`/`emit_handler` look up `None` for every handler id
        // and emit no `Patch::Handler` on a handler-body edit — so a `count + 1`
        // -> `count + 2` change ships no closure update and both hosts keep
        // running the stale init-time body (FLUX-014 regression).
        let mut arena = merged.unwrap_or_default();
        for c in &closures {
            arena.add_closure(c.clone());
        }
        // Re-key the merged tree to content-addressed ids (FLUX-074, item A).
        // The wire path consumes ids as opaque u32; only *which* u32 each node
        // gets changes. `content_address` itself; the pipeline's *external*
        // `prop_thunks` table is keyed by the same node ids, so it must be
        // remapped in lockstep from the returned old→new map.
        let remap = arena.content_address();
        prop_thunks = prop_thunks
            .into_iter()
            .map(|(id, thunk)| (remap.get(&id).copied().unwrap_or(id), thunk))
            .collect();
        Ok(TreeCompilation {
            arena,
            closures,
            state_seed,
            sources,
            prop_thunks,
            component_names,
            monomorphizations,
        })
    }

    /// Builds a module loader that resolves `use theme` to a file under the
    /// package root: `<root>/theme.flux`, then `<root>/theme/main.flux`.
    ///
    /// The loader is `Send + Sync` so it can be shared with the sub-checkers the
    /// type checker spins up for transitive `use`s.
    fn module_loader(&self) -> ModuleLoader {
        let root = self.root.clone();
        Arc::new(move |name: &str| {
            let direct = root.join(format!("{name}.flux"));
            if direct.is_file() {
                return std::fs::read_to_string(&direct).ok();
            }
            let nested = root.join(name).join("main.flux");
            if nested.is_file() {
                return std::fs::read_to_string(&nested).ok();
            }
            None
        })
    }

    /// Compiles one file through parse → type check → lower.
    ///
    /// Returns the lowered IR and the original parsed AST (the AST is needed by
    /// the release codegen path, which recovers names and semantics from it).
    fn compile_one(
        &mut self,
        source: &str,
        file_id: FileId,
        display: &str,
    ) -> Result<(LoweredIr, Ast), Diagnostic> {
        let started = Instant::now();
        let ast = flux_parser::parse(source, file_id, display)
            .map_err(|e| Diagnostic::from(FluxError::from(e)))?;
        self.timings.parse = started.elapsed();

        let started = Instant::now();
        let loader = self.module_loader();
        let typed = flux_types::type_check_with_loader(&ast, Some(loader))
            .map_err(|e| Diagnostic::from(FluxError::from(e)))?;
        self.timings.type_check = started.elapsed();

        let started = Instant::now();
        let lowered = lower(&ast, &typed).map_err(|e| {
            let flux_ir::LoweringError::Lower { message, span } = e;
            // `LoweringError` lives in `flux-ir`, which depends on `flux-types`, so
            // a `From` into `FluxError` would be a cycle; build the `Compile`
            // variant directly instead (AGENTS.md §3.5 / LANE-I).
            let err = FluxError::Compile(CompileError::from_lowering(message, span));
            Diagnostic::from(err)
        })?;
        self.timings.lower = started.elapsed();
        Ok((lowered, ast))
    }

    /// Builds the `Init` frame bytes for `arena` (spec §D.12.2).
    fn build_init(
        &mut self,
        arena: &IRArena,
        closures: &[flux_ir::ClosureIR],
        state_seed: &[(SignalId, Value)],
        component_names: &[(flux_syntax::ComponentId, String)],
    ) -> Vec<u8> {
        let root = root_node(arena);
        // Appendix D §D.12.2: the Init frame carries `root` followed by every
        // descendant node, flat, so a host rebuilds the full node table from one
        // frame. Gather the descendants (children first, breadth-first) here.
        let extra_nodes = flatten_extra_nodes(&root, arena);
        let source_map = self.source_map();
        let signal_meta = signal_meta_for(arena);
        let mut frame: InitFrame = Frame::init(
            &root,
            &extra_nodes,
            state_seed,
            &source_map,
            arena.string_table(),
            component_names,
            closures,
            &signal_meta,
        );
        self.seq = self.seq.wrapping_add(1);
        frame.seq = self.seq;
        // Reuse the scratch buffer (OPT-B) so the per-connection Init path is
        // allocation-free after warm-up; a reconnecting host always receives the
        // full string table (ids are per-compile positional, not content-stable).
        frame.encode_into(&mut self.scratch);
        self.scratch.clone()
    }

    /// Builds the `Delta` frame bytes for `patches` (spec §D.1).
    fn build_delta(
        &mut self,
        arena: &IRArena,
        patches: &[Patch],
        closures: &[flux_ir::ClosureIR],
    ) -> Vec<u8> {
        // The full arena string table is shipped on every Delta. String ids are
        // assigned densely *per compile* (flux_syntax::StringTable interns in
        // first-insertion order, not content-derived), so an edit that shifts
        // the table reassigns every id. A host that merges deltas into the table
        // it already holds would otherwise bind a stale id→text pair and render
        // wrong text. Shipping the whole table keeps the id→text mapping correct;
        // the scratch buffer (OPT-B) keeps this allocation-free after warm-up.
        let strings: Vec<(flux_syntax::StringId, String)> = arena
            .string_table()
            .iter()
            .map(|(id, text)| (id, text.to_owned()))
            .collect();
        let signal_meta = signal_meta_for(arena);
        // Set the ADR-0027 flag only when this frame actually carries metadata,
        // so hosts without the gate still read a valid (metadata-less) frame.
        let flags = if signal_meta.is_empty() {
            flux_ir_serde::FLAG_HAS_STRING_DELTA
        } else {
            flux_ir_serde::FLAG_HAS_STRING_DELTA | flux_ir_serde::FLAG_NODE_HAS_SIGNAL_DEPS
        };
        self.seq = self.seq.wrapping_add(1);
        let frame = Frame::delta(self.seq, flags, patches, &strings, closures, &signal_meta);
        frame.encode_into(&mut self.scratch);
        self.scratch.clone()
    }

    /// Rebuilds the `Init` frame from the retained good tree, for a
    /// (re)connecting host. Returns `None` before the first good compile.
    #[must_use]
    pub fn init_frame(&mut self) -> Option<Vec<u8>> {
        let (arena, closures, state_seed, component_names) = {
            let last = self.last_good.as_ref()?;
            // The retained closure table already includes the prop thunks (they
            // are folded in when the tree advances), so a reconnecting host gets
            // the same handler blob the original `Init` carried.
            (
                last.arena.clone(),
                last.closures.values().cloned().collect::<Vec<_>>(),
                last.state_seed.clone(),
                last.component_names.clone(),
            )
        };
        Some(self.build_init(&arena, &closures, &state_seed, &component_names))
    }

    /// Builds an `Error` frame from `diagnostic` (spec §D.12.3).
    ///
    /// When the diagnostic carries a `Span` and the server still holds the source
    /// text for that file, an ADR-0057 `SourceExcerpt` (path:line:col + the
    /// offending line) is computed once here and shipped inline, so the host can
    /// render a snippet/caret without re-scanning source or a round-trip.
    pub fn error_frame(&mut self, diagnostic: &Diagnostic) -> Vec<u8> {
        self.seq = self.seq.wrapping_add(1);
        let excerpt = diagnostic
            .span
            .and_then(|span| self.sources.get(&span.file_id).map(|(_, src)| (span, src)))
            .and_then(|(span, src)| SourceExcerpt::from_span(span, src));
        Frame::error(self.seq, &diagnostic.message, diagnostic.span, excerpt).to_bytes()
    }
}

/// Builds the ADR-0027 (FA-IRWIRE) per-node `signal_meta` section for `arena`:
/// one [`NodeSignalMeta`] per node that carries `signal_deps` (T13), including
/// its optional `prop_thunk` (T14) and `prop_layout` (T14). Only nodes with
/// non-empty metadata are emitted, keeping the frame compact when no node reads
/// a signal.
///
/// The prop-thunk `ClosureRef` ships its content `hash` but no `bytecode_offset`:
/// hosts resolve thunk bytecode by hash from the frame's handler table (the
/// shared blob is sliced per-handler by the host), so the two native hosts
/// (iOS, Android) share one resolution rule and never drift (parity contract,
/// Appendix F). Emitting a `bytecode_offset` here would force offset-based
/// slicing and diverge from iOS.
fn signal_meta_for(arena: &IRArena) -> Vec<NodeSignalMeta> {
    let mut metas = Vec::new();
    for id in arena.all_ids() {
        let deps = arena.signal_deps_of(id);
        if deps.is_empty() {
            continue;
        }
        let thunk = arena.prop_thunk_of(id).cloned();
        let layout = arena.prop_layout_of(id).to_vec();
        let item_slot = arena.item_slot_of(id);
        metas.push(NodeSignalMeta {
            node_id: id,
            deps: deps.to_vec(),
            thunk,
            layout,
            item_slot,
        });
    }
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir::{ClosureIR, InstanceRegistry, LoweredIr};
    use flux_parser::Decl;
    use flux_syntax::Span;

    #[test]
    fn compiled_sources_is_empty_before_first_compile() {
        let pipeline = Pipeline::new("/tmp/project", false);
        assert!(pipeline.compiled_sources().is_empty());
    }

    #[test]
    fn compiled_sources_returns_one_entry_per_file_with_consistent_ir_and_ast() {
        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n".to_owned(),
        );

        let outcome = pipeline.compile().expect("well-formed source compiles");
        assert!(matches!(outcome, Compiled::Init(_)));

        let sources = pipeline.compiled_sources();
        assert_eq!(sources.len(), 1, "one entry per tracked .flux file");

        let (path, ir, ast) = &sources[0];
        assert_eq!(path, Path::new("/tmp/project/main.flux"));
        assert!(
            !ir.arena.is_empty(),
            "lowered arena must carry at least one node"
        );
        assert_eq!(
            ast.decls.len(),
            1,
            "ast must retain the parsed component declaration"
        );
        assert!(
            matches!(ast.decls[0], Decl::Component(_)),
            "ast must carry the expected component"
        );
    }

    #[test]
    fn content_addressed_ids_survive_a_text_above_edit() {
        // FLUX-074, item A: inserting source text *above* a node shifts its span
        // but leaves its structural content unchanged, so its wire-tree NodeId must
        // not change. The devserver content-addresses the merged tree, which is what
        // lets the differ's state-preserving `Reattach` keep the host view alive
        // across a hot reload instead of tearing it down and rebuilding it.
        let before = "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n";
        let after = "// a leading comment that pushes everything below it down\n\ncompo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n";

        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(Path::new("/tmp/project/main.flux"), before.to_owned());
        pipeline.compile().expect("before compiles");
        let ids_before: Vec<flux_syntax::NodeId> = pipeline
            .last_good
            .as_ref()
            .unwrap()
            .arena
            .all_ids()
            .collect();

        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(Path::new("/tmp/project/main.flux"), after.to_owned());
        pipeline.compile().expect("after compiles");
        let ids_after: Vec<flux_syntax::NodeId> = pipeline
            .last_good
            .as_ref()
            .unwrap()
            .arena
            .all_ids()
            .collect();

        assert_eq!(
            ids_before, ids_after,
            "a pure text-above edit must not change any wire-tree NodeId (FLUX-074)"
        );
    }

    #[test]
    fn incremental_lowering_only_re_lowers_changed_files() {
        // FLUX-074, item B: a recompile that edits one of several files must
        // re-parse / re-type-check / re-lower only the changed file. Unchanged
        // files are reused from the per-file cache, so `lower_count` counts just
        // the one edit — bounded work, not whole-program re-lower.
        use std::path::Path;

        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/a.flux"),
            "compo A\n  Text(\"a\")\n".to_owned(),
        );
        pipeline.set_source(
            Path::new("/tmp/project/b.flux"),
            "compo B\n  Text(\"b\")\n".to_owned(),
        );
        pipeline.compile().expect("first compile succeeds");
        assert_eq!(
            pipeline.lower_count(),
            2,
            "both files lowered the first time"
        );

        // Edit only `b.flux`. `a.flux` is byte-identical, so it must be reused.
        pipeline.set_source(
            Path::new("/tmp/project/b.flux"),
            "compo B\n  Text(\"b edited\")\n".to_owned(),
        );
        pipeline.compile().expect("recompile succeeds");
        assert_eq!(
            pipeline.lower_count(),
            1,
            "only the changed file is re-lowered (FLUX-074 item B)"
        );

        // A no-op recompile (sources unchanged) reuses both from cache.
        pipeline.compile().expect("noop recompile succeeds");
        assert_eq!(
            pipeline.lower_count(),
            0,
            "unchanged sources are not re-lowered at all"
        );
    }

    #[test]
    fn compiled_sources_is_cleared_on_failed_compile_retaining_previous_wire_tree() {
        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Hello\n  Text(\"hi\")\n".to_owned(),
        );
        pipeline.compile().expect("first compile succeeds");
        assert_eq!(pipeline.compiled_sources().len(), 1);

        // A broken recompile leaves the wire tree intact but the codegen store
        // must reflect that no new good sources were produced this round.
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Broken\n  state x: Int = false\n".to_owned(),
        );
        let result = pipeline.compile();
        assert!(result.is_err(), "malformed source fails to compile");
        assert!(
            pipeline.has_tree(),
            "previous good tree is retained for the wire"
        );
        assert_eq!(
            pipeline.compiled_sources().len(),
            1,
            "codegen store keeps the last good sources"
        );
    }

    #[test]
    fn required_capabilities_extracts_call_cap_pairs() {
        // A handler closure whose bytecode CALL_CAPs cap 2 method 2 and cap 3
        // method 1, plus a prop thunk CALL_CAPing cap 2 method 2 again (deduped),
        // and a trailing HALT.
        let call_cap = |cap_id: u32, method_id: u16| -> Vec<u8> {
            let mut b = vec![0x90u8, 0x00];
            b.extend_from_slice(&cap_id.to_le_bytes());
            b.extend_from_slice(&method_id.to_le_bytes());
            b.push(0x00); // args_reg
            b
        };
        let mut bytecode = call_cap(2, 2);
        bytecode.extend_from_slice(&call_cap(3, 1));
        bytecode.push(0x00); // HALT

        let thunk_bytecode = call_cap(2, 2); // duplicate of cap 2 method 2

        let ir = LoweredIr {
            arena: IRArena::new(),
            closures: std::collections::HashMap::from([(
                flux_syntax::HandlerId::from(1u32),
                ClosureIR::new(
                    flux_syntax::HandlerId::from(1u32),
                    bytecode,
                    Vec::new(),
                    Span::new(0, 0, 0),
                ),
            )]),
            prop_thunks: std::collections::HashMap::from([(
                flux_syntax::NodeId::from(2u32),
                ClosureIR::new(
                    flux_syntax::HandlerId::from(2u32),
                    thunk_bytecode,
                    Vec::new(),
                    Span::new(0, 0, 0),
                ),
            )]),
            state_seed: Vec::new(),
            component_names: Vec::new(),
            monomorphizations: Vec::new(),
            instances: InstanceRegistry::new(),
        };

        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.last_good = Some(ir);
        let required = pipeline.required_capabilities();
        assert_eq!(required, vec![(2, 2), (3, 1)], "CALL_CAP pairs, deduped");
    }

    /// A generic `Counter` and one call site, self-contained: the pipeline
    /// type-checks each file independently, so a cross-file reference is an
    /// unbound name. The merge is still what's under test — each file lowers its
    /// own instantiation and the tree must collect all of them.
    fn generic_file(component: &str, initial: &str) -> String {
        format!(
            "trait Numeric[T] {{\n  fn zero() -> T\n  fn one() -> T\n}}\n\ncompo Counter[T: Numeric](initial: T)\n  state count: T = initial\n\n\ncompo {component}\n  Counter(initial: {initial})\n\n"
        )
    }

    #[test]
    fn monomorphizations_are_empty_before_the_first_compile() {
        let pipeline = Pipeline::new("/tmp/project", false);
        assert!(pipeline.monomorphizations().is_empty());
    }

    #[test]
    fn a_program_without_generics_has_no_monomorphizations() {
        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Hello\n  Button(text: \"tap\")\n".to_owned(),
        );
        pipeline.compile().expect("compiles");
        assert!(
            pipeline.monomorphizations().is_empty(),
            "no generic call sites means nothing to specialise"
        );
    }

    #[test]
    fn the_tree_merge_collects_instantiations_from_every_file() {
        // Phase 1's pipeline task: instantiations discovered in separate files
        // must all survive into the merged tree, or a backend emits only the
        // types it happened to see in one file.
        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/int.flux"),
            generic_file("IntCase", "0"),
        );
        pipeline.set_source(
            Path::new("/tmp/project/float.flux"),
            generic_file("FloatCase", "0.0"),
        );
        pipeline.compile().expect("the generic program compiles");

        let mut names: Vec<String> = pipeline
            .monomorphizations()
            .into_iter()
            .map(|mono| mono.mangled)
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names,
            vec!["Counter_Float", "Counter_Int"],
            "both instantiations must reach the merged tree"
        );
    }

    #[test]
    fn merged_instantiations_are_deduplicated() {
        // Two files instantiating `Counter[Int]` must not yield two identical
        // entries, or the backend would emit the same native type twice.
        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(Path::new("/tmp/project/a.flux"), generic_file("ACase", "0"));
        pipeline.set_source(Path::new("/tmp/project/b.flux"), generic_file("BCase", "0"));
        pipeline.compile().expect("compiles");

        let monos = pipeline.monomorphizations();
        let int_entries = monos
            .iter()
            .filter(|mono| mono.mangled == "Counter_Int")
            .count();
        assert_eq!(
            int_entries, 1,
            "the same instantiation in two files is one specialisation"
        );
    }

    #[test]
    fn delta_ships_full_string_table_for_positional_ids() {
        // Correctness guard (not a micro-opt): `flux_syntax::StringTable` interns
        // ids densely in first-insertion order *per compile*, so an edit that
        // shifts the table reassigns every id. A host applies a Delta by mapping
        // string ids to text, so the Delta must carry the FULL table — not a
        // partial delta — or the host would bind a stale id→text pair and render
        // wrong text. (A tempting "ship only new strings" optimization is unsafe
        // precisely because the ids are not content-stable across compiles.)
        //
        // Note (FLUX-074): a *content-preserving* literal edit no longer churns
        // ids — content-addressed wire IDs keep the node stable, so a pure text
        // edit that preserves content yields `Unchanged` rather than a churned
        // `Delta`. That is the desired hot-reload win. A genuine *structural* edit
        // still ships a `Delta`, and that Delta carries the complete table for the
        // new compile.
        use flux_ir_serde::Frame;

        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Hello\n  Button(text: \"first\")\n".to_owned(),
        );
        let init = match pipeline.compile().expect("compiles") {
            Compiled::Init(bytes) => bytes,
            other => panic!("first compile is an Init, got {other:?}"),
        };
        let init_frame = Frame::from_init_bytes(&init).expect("decodes as Init");
        let init_string_count = init_frame.string_table.len();

        // Content-preserving literal edit: "first" → "second". Same structure,
        // same content hash (string ids are positional, so the prop value is
        // identical), so FLUX-074 keeps the wire id stable and the compiler
        // ships `Unchanged` instead of a churned Delta.
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Hello\n  Button(text: \"second\")\n".to_owned(),
        );
        match pipeline.compile().expect("recompiles") {
            Compiled::Unchanged => {}
            other => panic!(
                "content-preserving literal edit keeps wire id stable → Unchanged, got {other:?}"
            ),
        }

        // Genuine structural edit: add a second child node. The compiler must
        // ship a `Delta`, and that Delta carries the complete (full) string
        // table for the new compile, not a partial one.
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "compo Hello\n  Button(text: \"first\")\n  Button(text: \"second\")\n".to_owned(),
        );
        let delta = match pipeline.compile().expect("recompiles") {
            Compiled::Delta(bytes) => bytes,
            other => panic!("structural edit ships a Delta, got {other:?}"),
        };
        let delta_frame = Frame::from_delta_bytes(&delta).expect("decodes as Delta");
        // The Delta must carry the *full* string table for this compile — the
        // same table the arena holds — not a partial subset (positional ids are
        // per-compile, so dropping any entry would bind a stale id→text pair).
        let full_table_len = pipeline
            .last_good
            .as_ref()
            .expect("tree compiled")
            .arena
            .string_table()
            .len();
        assert_eq!(
            delta_frame.strings.len(),
            full_table_len,
            "Delta ships the full table (positional ids are per-compile, not content-stable)"
        );
        let _ = init_string_count;
    }

    #[test]
    fn use_resolves_module_from_package_root_on_disk() {
        use flux_types::type_check_with_loader;
        use std::sync::Arc;

        let dir = std::env::temp_dir().join(format!("flux_use_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("theme.flux"),
            "compo Button(label: String)\n  Text(label)\n",
        )
        .expect("write theme.flux");
        let entry = "use theme\n\ncompo Main()\n  Button(label: \"hi\")\n";

        // Mirror the dev server's package-root loader: `<root>/<name>.flux`.
        let root = dir.clone();
        let loader: ModuleLoader = Arc::new(move |name: &str| {
            let direct = root.join(format!("{name}.flux"));
            if direct.is_file() {
                return std::fs::read_to_string(&direct).ok();
            }
            let nested = root.join(name).join("main.flux");
            if nested.is_file() {
                return std::fs::read_to_string(&nested).ok();
            }
            None
        });

        let ast = flux_parser::parse(entry, 0, "main").expect("entry parses");
        let typed = type_check_with_loader(&ast, Some(loader));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            typed.is_ok(),
            "use theme should resolve theme.flux from the package root: {:?}",
            typed.err()
        );
    }

    #[test]
    fn perf_records_empty_before_first_compile() {
        // `perf_records` must not fabricate data before a compile has recorded
        // timings (no fake flamegraph bars before the server has measured).
        let mut pipeline = Pipeline::new(".", false);
        pipeline.set_source(
            Path::new("/tmp/perf_test/main.flux"),
            "compo Hello\n  Button(text: \"hi\")\n".to_owned(),
        );
        assert!(pipeline.perf_records().is_empty());
    }

    #[test]
    fn perf_records_reports_save_to_photon_and_patch_round_trip() {
        // After a real compile, `perf_records` emits the server-side Save→pixels
        // breakdown the DevTools flamegraph consumes (FLUX-059 / PRD-J): the full
        // pipeline as SaveToPhoton and the serialize leg as PatchRoundTrip.
        let mut pipeline = Pipeline::new(".", false);
        pipeline.set_source(
            Path::new("/tmp/perf_test/main.flux"),
            "compo Hello\n  Button(text: \"hi\")\n".to_owned(),
        );
        pipeline.compile().expect("compiles");
        let records = pipeline.perf_records();
        assert_eq!(records.len(), 2);
        let kinds: Vec<_> = records.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&MetricKind::SaveToPhoton));
        assert!(kinds.contains(&MetricKind::PatchRoundTrip));
        // Every record is a valid, parseable MetricRecord document.
        for rec in &records {
            assert!(rec.scenario == Scenario::LoopbackE2e);
            let json = rec.to_json().expect("serialize");
            let back = MetricRecord::from_json(&json).expect("round-trip");
            assert_eq!(rec, &back);
            assert!(!rec.samples.is_empty(), "a record must carry a sample");
        }
    }
}
