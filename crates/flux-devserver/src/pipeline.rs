//! The compile pipeline: source → parse → type check → lower → diff → frame.
//!
//! [`Pipeline`] owns the last-good [`LoweredIr`] for the project. Compiling a
//! source snapshot either advances that state and yields the frame to ship, or
//! returns a [`Diagnostic`] — in which case the previous good tree is retained
//! and no `Delta` is produced (spec §D.12.3).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use flux_ir::{IRArena, LoweredIr, lower};
use flux_ir_serde::{Frame, InitFrame};
use flux_syntax::{FileId, Patch};

use crate::error::Diagnostic;
use tree::{display_path, merge_arenas, root_node};

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
    /// Monotonic wire sequence number.
    seq: u32,
    profile: bool,
    /// Timings of the most recent compile.
    timings: PhaseTimings,
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
            seq: 0,
            profile,
            timings: PhaseTimings::default(),
        }
    }

    /// Per-phase timings of the most recent compile.
    #[must_use]
    pub fn timings(&self) -> PhaseTimings {
        self.timings
    }

    /// Whether a good tree has been compiled at least once.
    #[must_use]
    pub fn has_tree(&self) -> bool {
        self.last_good.is_some()
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
        let (arena, closures) = self.compile_tree()?;
        let started = Instant::now();
        let outcome = match self.last_good.as_ref() {
            None => {
                let frame = self.build_init(&arena, &closures);
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
            instances: flux_ir::InstanceRegistry::new(),
        });
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
    /// per-file arenas into a single tree.
    fn compile_tree(&mut self) -> Result<(IRArena, Vec<flux_ir::ClosureIR>), Diagnostic> {
        let snapshots: Vec<(FileId, PathBuf, String)> = self
            .sources
            .iter()
            .map(|(id, (path, src))| (*id, path.clone(), src.clone()))
            .collect();
        let mut merged: Option<IRArena> = None;
        let mut closures: Vec<flux_ir::ClosureIR> = Vec::new();
        for (file_id, path, source) in snapshots {
            let display = display_path(&self.root, &path);
            let lowered = self.compile_one(&source, file_id, &display)?;
            closures.extend(lowered.closures.values().cloned());
            merged = Some(match merged {
                None => lowered.arena,
                // Multi-file projects merge by packing the later file's nodes
                // into the first arena; node IDs are content-derived so the
                // merge cannot collide across files with distinct spans.
                Some(base) => merge_arenas(base, &lowered.arena),
            });
        }
        Ok((merged.unwrap_or_default(), closures))
    }

    /// Compiles one file through parse → type check → lower.
    fn compile_one(
        &mut self,
        source: &str,
        file_id: FileId,
        display: &str,
    ) -> Result<LoweredIr, Diagnostic> {
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
        Ok(lowered)
    }

    /// Builds the `Init` frame bytes for `arena` (spec §D.12.2).
    fn build_init(&mut self, arena: &IRArena, closures: &[flux_ir::ClosureIR]) -> Vec<u8> {
        let root = root_node(arena);
        let source_map = self.source_map();
        let mut frame: InitFrame =
            Frame::init(&root, &[], &source_map, arena.string_table(), closures);
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
        self.seq = self.seq.wrapping_add(1);
        Frame::delta(
            self.seq,
            flux_ir_serde::FLAG_HAS_STRING_DELTA,
            patches,
            &strings,
            closures,
        )
        .to_bytes()
    }

    /// Rebuilds the `Init` frame from the retained good tree, for a
    /// (re)connecting host. Returns `None` before the first good compile.
    #[must_use]
    pub fn init_frame(&mut self) -> Option<Vec<u8>> {
        let (arena, closures) = {
            let last = self.last_good.as_ref()?;
            (
                last.arena.clone(),
                last.closures.values().cloned().collect::<Vec<_>>(),
            )
        };
        Some(self.build_init(&arena, &closures))
    }

    /// Builds an `Error` frame from `diagnostic` (spec §D.12.3).
    pub fn error_frame(&mut self, diagnostic: &Diagnostic) -> Vec<u8> {
        self.seq = self.seq.wrapping_add(1);
        Frame::error(self.seq, &diagnostic.message, diagnostic.span).to_bytes()
    }
}
