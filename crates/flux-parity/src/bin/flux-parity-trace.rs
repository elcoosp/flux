//! `flux-parity-trace` — T16 trace-diff CLI (ADR-0027 / reconcile-trace-format v1).
//!
//! Usage:
//! ```text
//! flux-parity-trace trace diff --phase 1 trace.swift.jsonl trace.kotlin.jsonl
//! flux-parity-trace trace diff trace.swift.jsonl trace.kotlin.jsonl
//! ```
//!
//! Exit codes: `0` when the two traces match exactly; `1` on the first
//! divergence; `2` on a usage / I/O / parse error.

use std::path::PathBuf;
use std::process::ExitCode;

use flux_parity::trace::{Phase, TraceError, diff_traces, phase_from_filename};

const EXIT_DIVERGENCE: u8 = 1;
const EXIT_USAGE: u8 = 2;

/// CLI subcommands.
#[derive(Debug)]
enum Command {
    /// `trace diff` — canonicalize and compare two phase traces.
    TraceDiff {
        /// Optional explicit phase; inferred from filenames when omitted.
        phase: Option<Phase>,
        /// Left trace (e.g. Swift output).
        left: PathBuf,
        /// Right trace (e.g. Kotlin output).
        right: PathBuf,
    },
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(Command::TraceDiff { phase, left, right }) => run_diff(phase, &left, &right),
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!(
                "usage: flux-parity-trace trace diff [--phase <1|2|3>] <left.jsonl> <right.jsonl>"
            );
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Parses the command line into a [`Command`].
///
/// # Errors
/// Returns a usage message string when the arguments are malformed.
fn parse(args: &[String]) -> Result<Command, String> {
    let mut it = args.iter();
    match it.next() {
        Some(cmd) if cmd == "trace" => parse_trace(it),
        Some(other) => Err(format!("unknown command: {other}")),
        None => Err("missing command".to_owned()),
    }
}

/// Parses `trace <subcommand> …`.
///
/// # Errors
/// Returns a usage message string when the subcommand or its arguments are malformed.
fn parse_trace<'a, I>(mut it: I) -> Result<Command, String>
where
    I: Iterator<Item = &'a String>,
{
    match it.next() {
        Some(sub) if sub == "diff" => parse_diff(it),
        Some(other) => Err(format!("unknown trace subcommand: {other}")),
        None => Err("missing trace subcommand".to_owned()),
    }
}

/// Parses `trace diff [--phase <p>] <left> <right>`.
///
/// # Errors
/// Returns a usage message string when the flags or positional arguments are malformed.
fn parse_diff<'a, I>(mut it: I) -> Result<Command, String>
where
    I: Iterator<Item = &'a String>,
{
    let mut phase: Option<Phase> = None;
    let mut positional: Vec<String> = Vec::new();
    while let Some(arg) = it.next() {
        if arg == "--phase" {
            let value = it.next().ok_or("--phase requires a value")?;
            let p: Phase = value
                .parse()
                .map_err(|_: TraceError| format!("invalid --phase: {value}"))?;
            phase = Some(p);
        } else {
            positional.push(arg.clone());
        }
    }
    if positional.len() != 2 {
        return Err("expected exactly two trace paths".to_owned());
    }
    let left = PathBuf::from(&positional[0]);
    let right = PathBuf::from(&positional[1]);
    Ok(Command::TraceDiff { phase, left, right })
}

/// Runs the `trace diff` command and maps its result to a process exit code.
fn run_diff(phase: Option<Phase>, left: &std::path::Path, right: &std::path::Path) -> ExitCode {
    // Phase is advisory: when omitted, infer from the filenames so the operator
    // gets a clear mismatch message if they compare across phases.
    if phase.is_none() {
        let lp = phase_from_filename(left);
        let rp = phase_from_filename(right);
        if lp != rp {
            eprintln!(
                "warning: trace phases differ (left={:?}, right={:?}); comparing anyway",
                lp.map(|p| p.to_string()),
                rp.map(|p| p.to_string())
            );
        }
    }
    match diff_traces(left, right) {
        Ok(()) => {
            println!("traces match ({} frames)", count_lines(left));
            ExitCode::SUCCESS
        }
        Err(TraceError::Divergence(report)) => {
            eprintln!("{report}");
            ExitCode::from(EXIT_DIVERGENCE)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Counts the non-empty lines in a trace file (best-effort; for the success message).
fn count_lines(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path)
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}
