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
}

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use flux_ir::{IRArena, LoweredIr, lower};
use flux_ir_serde::{Frame, InitFrame, NodeSignalMeta};
use flux_parser::Ast;
use flux_syntax::{FileId, Patch, SignalId, StringId, Value};

use crate::dispatch::{DependencyIndex, DispatchReport, NodeSignalDeps, emit_minimal_updates};
use crate::error::Diagnostic;
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
        }
    }

    /// Per-phase timings of the most recent compile.
    #[must_use]
    pub fn timings(&self) -> PhaseTimings {
        self.timings
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

    /// Builds the DevTools [`SourceMap`] from the last-good lowered IR so the
    /// debug bridge can enrich telemetry with `.flux` source spans (Phase 3).
    ///
    /// Prefers [`last_good`] (the retained tree); falls back to the first
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
        for (file_id, path, source) in snapshots {
            let display = display_path(&self.root, &path);
            let (lowered, ast) = self.compile_one(&source, file_id, &display)?;
            closures.extend(lowered.closures.values().cloned());
            closures.extend(lowered.prop_thunks.values().cloned());
            prop_thunks.extend(
                lowered
                    .prop_thunks
                    .iter()
                    .map(|(id, thunk)| (*id, thunk.clone())),
            );
            component_names.extend(lowered.component_names.iter().cloned());
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
        Ok(TreeCompilation {
            arena: merged.unwrap_or_default(),
            closures,
            state_seed,
            sources,
            prop_thunks,
            component_names,
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
        let ast = flux_parser::parse(source, file_id, display).map_err(|e| {
            Diagnostic::new(
                format!("parse error in {display}: {}", e.render()),
                Some(e.span),
            )
        })?;
        self.timings.parse = started.elapsed();

        let started = Instant::now();
        let typed = flux_types::type_check(&ast).map_err(|e| {
            Diagnostic::new(
                format!("type error in {display}: {}", e.message),
                Some(e.span),
            )
        })?;
        self.timings.type_check = started.elapsed();

        let started = Instant::now();
        let lowered = lower(&ast, &typed).map_err(|e| match e {
            flux_ir::LoweringError::Lower { message, span } => Diagnostic::new(
                format!("lowering error in {display}: {message}"),
                Some(span),
            ),
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
        frame.to_bytes()
    }

    /// Builds the `Delta` frame bytes for `patches` (spec §D.1).
    fn build_delta(
        &mut self,
        arena: &IRArena,
        patches: &[Patch],
        closures: &[flux_ir::ClosureIR],
    ) -> Vec<u8> {
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
        Frame::delta(self.seq, flags, patches, &strings, closures, &signal_meta).to_bytes()
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
    pub fn error_frame(&mut self, diagnostic: &Diagnostic) -> Vec<u8> {
        self.seq = self.seq.wrapping_add(1);
        Frame::error(self.seq, &diagnostic.message, diagnostic.span).to_bytes()
    }
}

/// Builds the ADR-0027 (FA-IRWIRE) per-node `signal_meta` section for `arena`:
/// one [`NodeSignalMeta`] per node that carries `signal_deps` (T13), including
/// its optional `prop_thunk` (T14) and `prop_layout` (T14). Only nodes with
/// non-empty metadata are emitted, keeping the frame compact when no node reads
/// a signal.
fn signal_meta_for(arena: &IRArena) -> Vec<NodeSignalMeta> {
    let mut metas = Vec::new();
    for id in arena.all_ids() {
        let deps = arena.signal_deps_of(id);
        if deps.is_empty() {
            continue;
        }
        let thunk = arena.prop_thunk_of(id).cloned();
        let layout = arena.prop_layout_of(id).to_vec();
        metas.push(NodeSignalMeta {
            node_id: id,
            deps: deps.to_vec(),
            thunk,
            layout,
        });
    }
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_parser::Decl;

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
            "component Hello { state count: Int = 0 Button(text: \"tap\") }".to_owned(),
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
    fn compiled_sources_is_cleared_on_failed_compile_retaining_previous_wire_tree() {
        let mut pipeline = Pipeline::new("/tmp/project", false);
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "component Hello { Text(\"hi\") }".to_owned(),
        );
        pipeline.compile().expect("first compile succeeds");
        assert_eq!(pipeline.compiled_sources().len(), 1);

        // A broken recompile leaves the wire tree intact but the codegen store
        // must reflect that no new good sources were produced this round.
        pipeline.set_source(
            Path::new("/tmp/project/main.flux"),
            "component Broken {".to_owned(),
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
}
