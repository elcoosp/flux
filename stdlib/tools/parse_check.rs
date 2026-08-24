//! Parse-check driver for the Flux standard library (FLUX-015).
//!
//! Parses every `.flux` file passed on the command line with
//! `flux_parser::parse` and reports one line per file. Exits non-zero if any
//! file fails to parse, printing the parser's rendered diagnostic.
//!
//! This driver lives in `/stdlib` (the stdlib agent's owned directory) and is
//! compiled by `stdlib/parse-check.sh` with `rustc` against the workspace's
//! already-built `flux-parser` rlib. It is deliberately *not* a Cargo crate:
//! adding a manifest would modify the frozen workspace membership, which
//! `docs/agents-boundaries-contract.md` R2 forbids.

use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: parse_check <file.flux>...");
        return ExitCode::FAILURE;
    }
    let mut failures = 0usize;
    for (index, path) in paths.iter().enumerate() {
        match fs::read_to_string(path) {
            Ok(source) => match flux_parser::parse(&source, index as u32, path) {
                Ok(ast) => println!("ok    {path} ({} decls)", ast.decls.len()),
                Err(error) => {
                    failures += 1;
                    println!("FAIL  {path}");
                    println!("{}", error.render());
                }
            },
            Err(error) => {
                failures += 1;
                println!("FAIL  {path}: unreadable: {error}");
            }
        }
    }
    println!("\n{} file(s) checked, {} failure(s)", paths.len(), failures);
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
