# FLUX-F44 — `MetricRecord::mean()` panics when the sample sum overflows f64

**Status:** OPEN (documented; not fixed this round — the fix shape is
non-obvious)
**Found:** round-89 bug hunt (`flux-perf-harness` sweep)
**Severity:** low (real latencies are in the milliseconds, never near
`f64::MAX`, so the panic is unreachable in practice — but it is a
panicking arithmetic path inside a pure aggregator that the codebase's
own style forbids)

## Summary

`crates/flux-perf-harness/src/metric.rs::MetricRecord::mean` sums
per-sample latencies and divides by the count:

    let sum: f64 = self.samples.iter().map(|s| s.latency.as_f64()).sum();
    Some(LatencyMs::from_raw(sum / self.samples.len() as f64))

Each individual `LatencyMs` is guaranteed finite and non-negative by
`try_from_raw`, but the **sum** has no such guarantee. Two samples near
`f64::MAX` overflow the sum to `+inf`, and `LatencyMs::from_raw(+inf)`
panics with "called with invalid value".

That path is a *panic* inside a method documented as "The mean latency.
`None` when there are no samples." — a panic for non-empty samples is
not part of that contract.

## Reproduction

In `crates/flux-perf-harness/tests/probe_round89.rs` (deleted after
the probe run, but reproducible in three lines):

    let samples: Vec<MetricSample> = (0..1000)
        .map(|_| MetricSample::latency(LatencyMs::from_raw(f64::MAX / 2.0)))
        .collect();
    let rec = MetricRecord::new(Scenario::LoopbackE2e, MetricKind::VmDispatch, 1, samples);
    rec.mean(); // panics inside LatencyMs::from_raw

## What a fix needs

The mean of a set of finite non-negative f64 values is not guaranteed
finite in floating point — it can overflow. Options:

1. **Saturating sum via `fold`:** compute the sum with `f64::min(f64::MAX, …)`
   at each step. This changes the mean value for large inputs but
   prevents the panic. Simple, but the mean is no longer exact.

2. **Compute the mean with an incremental average** (Welford or
   running-mean), avoiding the sum-overflow problem entirely:

       let mut mean = 0.0;
       for (i, s) in samples.iter().enumerate() {
           mean += (s.latency.as_f64() - mean) / (i + 1) as f64;
       }

   This is O(n), numerically stable, and cannot overflow for finite
   inputs. It is what a "we care about p95 but also report mean" crate
   should use.

3. **Return `Option<LatencyMs>` that is `None` on non-finite mean:**
   keeps the current sum-based shape but documents the failure:

       LatencyMs::try_from_raw(sum / n).ok()

   The API already returns `Option`, so extending "None" to cover
   overflow is the smallest change.

Option 3 is the smallest diff. Option 2 is the cleanest engineering.

## Why filed, not fixed

The fix shape is a design call — the answer depends on whether the
harness's contract is "mean is exactly the arithmetic mean, or `None`"
(option 3) or "mean is a numerically stable running mean" (option 2).
Both are defensible; neither is obviously right. The next reader should
pick before editing.

## Regression test

None added — a `#[ignore]`d panic test would be a weird artefact, and
the correct assertion depends on which fix lands. Add a test with the
fix.
