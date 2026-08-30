//! Validates every stdlib `.flux` file parses cleanly (FLUX-015 / FLUX-037 /
//! FLUX-038 / FLUX-042).
//!
//! The type checker does not read the stdlib source (it reconstitutes the
//! prelude programmatically in `flux_types::prelude`), so the stdlib `.flux`
//! files are the human-authored contract for each primitive and must at least
//! parse. The `stdlib/parse-check.sh` harness normally gates this, but the
//! parser's rlib location drifts across toolchains; this Rust test exercises
//! the same `flux_parser::parse` entry point through the already-built crate
//! graph so CI always pins a parse regression in the stdlib.

use std::path::{Path, PathBuf};

/// Resolves the repo-root `stdlib/` directory from this crate's manifest dir.
fn stdlib_dir() -> PathBuf {
    // crate is at `<repo>/crates/flux-parity`; stdlib is `<repo>/stdlib`.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .expect("repo root")
        .join("stdlib")
}

/// Every stdlib `.flux` file must parse without error.
#[test]
fn all_stdlib_files_parse() {
    let dir = stdlib_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("stdlib dir readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "flux"))
        .collect();
    assert!(
        !files.is_empty(),
        "expected at least one .flux file in {}",
        dir.display()
    );
    files.sort();

    let mut failures = 0usize;
    for (index, path) in files.iter().enumerate() {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("FAIL  {} (unreadable: {e})", path.display());
                failures += 1;
                continue;
            }
        };
        match flux_parser::parse(&source, index as u32, &path.display().to_string()) {
            Ok(ast) => println!("ok    {} ({} decls)", path.display(), ast.decls.len()),
            Err(e) => {
                failures += 1;
                eprintln!("FAIL  {}", path.display());
                eprintln!("{e}");
            }
        }
    }
    assert_eq!(failures, 0, "stdlib .flux parse failures: {failures}");
}

/// The FLUX-037 / FLUX-038 / FLUX-042 primitives must each have a stdlib
/// declaration file (the issue required a stdlib declaration, not just a
/// codegen registry entry).
#[test]
fn required_primitive_declarations_exist() {
    let dir = stdlib_dir();
    for name in [
        "stack", "grid", "spacer", "safearea", // FLUX-037
        "modal", "sheet", "dialog",  // FLUX-038
        "animate", // FLUX-042
        "scrollview", // FLUX-056 (PRD-N `ScrollView`)
    ] {
        let path = dir.join(format!("{name}.flux"));
        assert!(
            path.exists(),
            "missing stdlib declaration for required primitive: {}",
            path.display()
        );
    }
}
