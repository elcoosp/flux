//! CI entry point for the on-device render-perf harness (FLUX-066).
//!
//! Unlike `ci_run.rs` (which builds a fixed warm *demonstration* record), this
//! example is the gate over **real** measurements produced by the host adapters
//! (`runtimes/android/host` and `runtimes/ios`). The host tests
//! (`RenderPerfHarnessTest` / `RenderPerfHarnessTests`) emit one or more
//! `MetricRecord`-shaped JSON documents (the same schema `metric::MetricRecord`
//! defines); this binary reads them from a file (one JSON object per line, or a
//! JSON array) and evaluates the §3.10 budget gate over each, exiting non-zero if
//! any record's p95 exceeds its ceiling.
//!
//! This is what makes the per-tier numbers *verified*, not asserted: the numbers
//! come from the real reconcilers (timing `reconcileDirty` on the JVM host and on
//! a booted iOS simulator), and the hard budget gate runs here in CI.

use std::fs;
use std::process::ExitCode;

use flux_perf_harness::{
    gate::{Budgets, evaluate},
    metric::MetricRecord,
};

/// Reads `MetricRecord` JSON from `path`. Accepts either a single JSON array of
/// records or newline-delimited JSON objects (one per line, as the host tests
/// print). Returns every record found.
fn load_records(path: &str) -> Result<Vec<MetricRecord>, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    // Try a JSON array first; fall back to newline-delimited objects.
    if let Ok(records) = serde_json::from_str::<Vec<MetricRecord>>(trimmed) {
        return Ok(records);
    }
    let mut out = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Tolerate the host `RENDER_PERF … json={…}` print form by extracting the
        // first balanced `{…}` payload.
        let payload = if let Some(start) = line.find('{') {
            let rest = &line[start..];
            match extract_balanced(rest) {
                Some(p) => p,
                None => line.to_string(),
            }
        } else {
            line.to_string()
        };
        match serde_json::from_str::<MetricRecord>(payload.as_str()) {
            Ok(r) => out.push(r),
            Err(e) => return Err(format!("parsing record from {path}: {e}")),
        }
    }
    Ok(out)
}

/// Extracts the first balanced `{…}` substring from `s` (ignoring string content
/// so embedded braces inside JSON strings don't confuse the scan).
fn extract_balanced(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_str = !in_str,
            b'{' if !in_str => {
                depth += 1;
            }
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    end.map(|e| s[..e].to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();

    if files.is_empty() {
        eprintln!("usage: ci_ondevice <record.json> [<record2.json> ...]");
        eprintln!("  each file holds MetricRecord JSON (array or newline-delimited).");
        return ExitCode::FAILURE;
    }

    let budgets = Budgets::v1();
    let mut any_failed = false;
    let mut total = 0usize;

    for path in files {
        let records = match load_records(path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                any_failed = true;
                continue;
            }
        };
        for rec in &records {
            total += 1;
            let verdict = evaluate(rec, &budgets);
            println!(
                "scenario={:?} kind={:?} tree_size={} samples={} p50={:?} p95={:?} -> {}",
                rec.scenario,
                rec.kind,
                rec.tree_size,
                rec.samples.len(),
                rec.p50().map(|l| l.as_f64()),
                rec.p95().map(|l| l.as_f64()),
                if verdict.passed { "PASS" } else { "FAIL" },
            );
            if !verdict.passed {
                eprintln!(
                    "  BUDGET EXCEEDED: {} (observed p95 {:.3}ms, ceiling {:.3}ms)",
                    verdict.reason, verdict.observed_p95, verdict.ceiling
                );
                any_failed = true;
            }
        }
    }

    if total == 0 {
        eprintln!("no MetricRecord documents were loaded");
        return ExitCode::FAILURE;
    }
    if any_failed {
        eprintln!("{total} record(s) evaluated; gate FAILED");
        ExitCode::FAILURE
    } else {
        println!("{total} record(s) evaluated; gate PASSED");
        ExitCode::SUCCESS
    }
}
