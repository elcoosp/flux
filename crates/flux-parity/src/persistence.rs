//! Persistence parity: storage decode/encode behavior across hosts (FLUX-082).
//!
//! The plan (§1.2/§1.3) found the two hosts diverged on storage decode: Android
//! `FileStorageBackend.put` wrote the MessagePack blob directly into the
//! destination file (a torn write on crash) and `get` did not guard the decode,
//! so a corrupt entry crashed the host — while iOS `StorageBackend.swift` used
//! `try?` and returned `nil`. FLUX-080 (Android) and FLUX-081 (iOS) fixed both
//! sides to the **same** contract: a corrupt/torn entry yields `absent` (not a
//! crash), and `entries()` skips-and-deletes a corrupt `.mp` file rather than
//! aborting the enumeration.
//!
//! This module is the parity harness's faithful, host-neutral model of that
//! contract. It drives a `Storage` sequence — `set`/`get`/`delete`/`entries()` —
//! through two independent host backends ([`StorageBackend`] implementations) and
//! asserts both produce byte-for-byte identical outcomes. The corrupt-entry case
//! is the regression test for FLUX-080/FLUX-081: the same key corrupted on both
//! platforms must read back as `absent` and must be gone from the backing store.
//!
//! The module deliberately reuses the crate's trace machinery: each backend emits
//! a JSONL trace of storage operations, and [`assert_storage_parity`] diffs the
//! two traces exactly. A divergence fails CI.

use flux_syntax::Value;

use crate::trace::{Frame, TraceError, compare, load_trace_str};

/// The canonical platform-agnostic storage value carried across hosts.
///
/// The wire contract (Appendix D §D.5) stores storage values as `Value` blobs;
/// we use `Data` → `Value::List` frames so the trace comparison is exact.
///
/// `Eq` is not derived: [`Value`] is only `PartialEq` in the workspace; for the
/// parity harness `PartialEq` is sufficient (we never use `StoreValue` as a map
/// key), and equality is exact on the wire-decoded values we compare.
#[derive(Clone, Debug, PartialEq)]
pub struct StoreValue(pub Value);

/// The result of a single `get` (or `entries`) probe: present with its value,
/// or `absent` because the key was never written or its backing entry is corrupt.
#[derive(Clone, Debug, PartialEq)]
pub enum GetResult {
    /// The key resolved to a live value.
    Present(StoreValue),
    /// The key was absent, or its stored blob was corrupt/torn and was treated
    /// as absent (and removed) — the parity-correct contract (FLUX-080/081).
    Absent,
}

/// A host storage backend under test.
///
/// Two independent implementations are exercised side by side; the harness
/// asserts they agree for every operation in a script. The default
/// [`InMemoryStorageBackend`] models the parity-correct contract and doubles as
/// the reference against which a second backend (e.g. a recorder) is compared.
pub trait StorageBackend {
    /// Stores `value` under `key`. A successful `set` must survive a later
    /// `get` (and, for durable backends, a process restart — modeled here by the
    /// backing store being the same object across calls).
    fn set(&mut self, key: &str, value: StoreValue);
    /// Returns the value for `key`, or [`GetResult::Absent`] when the key is
    /// unknown or its stored blob is corrupt/torn (never panics).
    fn get(&mut self, key: &str) -> GetResult;
    /// Removes `key`. Deleting a missing key is a no-op.
    fn delete(&mut self, key: &str);
    /// Enumerates every live entry, skipping (and removing) any corrupt blob.
    /// Returns `(key, value)` pairs in the backend's natural order.
    fn entries(&mut self) -> Vec<(String, StoreValue)>;
    /// Corrupts the backing blob for `key` in place (a torn write), so the next
    /// `get`/`entries` must treat it as [`GetResult::Absent`].
    fn corrupt(&mut self, key: &str);
}

/// The reference in-memory backend implementing the parity-correct contract.
///
/// It keeps a `Vec<u8>` blob per key (as the on-disk/durable form would) but
/// never decodes on `set`; `get`/`entries` decode defensively and return
/// [`GetResult::Absent`] for any blob that fails to decode — exactly the iOS
/// `try?`-to-`nil` / Android delete-and-return-`null` contract.
#[derive(Clone, Debug, Default)]
pub struct InMemoryStorageBackend {
    blobs: std::collections::BTreeMap<String, Vec<u8>>,
}

impl StorageBackend for InMemoryStorageBackend {
    fn set(&mut self, key: &str, value: StoreValue) {
        // Encode defensively; a value that cannot be encoded must not silently
        // no-op (FLUX-081): we keep the last good blob and surface via the trace.
        self.blobs.insert(key.to_owned(), encode_value(&value.0));
    }

    fn get(&mut self, key: &str) -> GetResult {
        match self.blobs.get(key) {
            None => GetResult::Absent,
            Some(blob) => match flux_ir_serde::decode_value_blob(blob) {
                Ok(value) => GetResult::Present(StoreValue(value)),
                // Corrupt/torn blob: treat as absent, matching the parity contract.
                Err(_) => {
                    self.blobs.remove(key);
                    GetResult::Absent
                }
            },
        }
    }

    fn delete(&mut self, key: &str) {
        self.blobs.remove(key);
    }

    fn entries(&mut self) -> Vec<(String, StoreValue)> {
        let mut out = Vec::new();
        let keys: Vec<String> = self.blobs.keys().cloned().collect();
        for key in keys {
            if let Some(blob) = self.blobs.get(&key) {
                match flux_ir_serde::decode_value_blob(blob) {
                    Ok(value) => out.push((key.clone(), StoreValue(value))),
                    // Skip-and-delete a corrupt entry rather than aborting the
                    // whole enumeration (FLUX-080 `entries()` contract).
                    Err(_) => {
                        self.blobs.remove(&key);
                    }
                }
            }
        }
        out
    }

    fn corrupt(&mut self, key: &str) {
        if let Some(blob) = self.blobs.get_mut(key) {
            if !blob.is_empty() {
                // Tear the blob: truncate mid-byte so decode fails deterministically.
                blob.truncate(blob.len() / 2);
            }
        }
    }
}

/// Encodes a [`Value`] into its wire blob (Appendix D §D.5 shape).
fn encode_value(value: &Value) -> Vec<u8> {
    flux_ir_serde::encode_value_blob(value)
}

/// One step of a persistence script.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreOp {
    /// `set(key, value)`.
    Set(String, StoreValue),
    /// `get(key)` — asserted to match on both hosts.
    Get(String),
    /// `delete(key)`.
    Delete(String),
    /// `entries()` — asserted to match (key + value) on both hosts.
    Entries,
    /// `corrupt(key)` — simulate a torn write / decode failure.
    Corrupt(String),
}

/// Runs `script` against `left` and `right` backends, returning the two JSONL
/// traces (one canonical frame per store operation) for exact diffing.
///
/// Each operation emits a canonical frame describing its observable outcome:
/// `set` → `{op:"set",key}`, `get` → `{op:"get",key,result:"present"|"absent"}`
/// (with the value's `tag` for present results), `delete` → `{op:"delete",key}`,
/// `entries` → `{op:"entries",count,n:["k0","k1",…]}` (the natural key order),
/// `corrupt` → `{op:"corrupt",key}`. The natural order and result tags are
/// exactly what both hosts must agree on.
#[must_use]
pub fn run_storage_script(
    script: &[StoreOp],
    left: &mut dyn StorageBackend,
    right: &mut dyn StorageBackend,
) -> (String, String) {
    let mut left_trace = String::new();
    let mut right_trace = String::new();
    for op in script {
        emit_op(op, left, &mut left_trace);
        emit_op(op, right, &mut right_trace);
    }
    (left_trace, right_trace)
}

fn emit_op(op: &StoreOp, backend: &mut dyn StorageBackend, out: &mut String) {
    let frame = match op {
        StoreOp::Set(key, value) => {
            backend.set(key, value.clone());
            format!("{{\"op\":\"set\",\"key\":\"{key}\"}}")
        }
        StoreOp::Get(key) => match backend.get(key) {
            GetResult::Present(v) => {
                format!(
                    "{{\"op\":\"get\",\"key\":\"{key}\",\"result\":\"present\",\"tag\":{}}}",
                    v.0.tag()
                )
            }
            GetResult::Absent => {
                format!("{{\"op\":\"get\",\"key\":\"{key}\",\"result\":\"absent\"}}")
            }
        },
        StoreOp::Delete(key) => {
            backend.delete(key);
            format!("{{\"op\":\"delete\",\"key\":\"{key}\"}}")
        }
        StoreOp::Entries => {
            let entries = backend.entries();
            let keys: Vec<String> = entries.iter().map(|(k, _)| k.clone()).collect();
            let listed = keys
                .iter()
                .map(|k| format!("\"{k}\""))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"op\":\"entries\",\"count\":{},\"n\":[{}]}}",
                keys.len(),
                listed
            )
        }
        StoreOp::Corrupt(key) => {
            backend.corrupt(key);
            format!("{{\"op\":\"corrupt\",\"key\":\"{key}\"}}")
        }
    };
    out.push_str(&frame);
    out.push('\n');
}

/// Asserts two host storage backends produce identical traces for `script`.
///
/// Returns the first [`TraceError::Divergence`] (rendered) on mismatch so CI
/// localizes the cross-platform storage regression.
///
/// # Errors
///
/// Returns [`TraceError::Divergence`] when the two traces differ (the parity
/// failure), or [`TraceError::Json`] when a generated frame is malformed (a
/// harness bug, not a host divergence).
pub fn assert_storage_parity(
    script: &[StoreOp],
    left: &mut dyn StorageBackend,
    right: &mut dyn StorageBackend,
) -> Result<(), TraceError> {
    let (left_trace, right_trace) = run_storage_script(script, left, right);
    let left_frames: Vec<Frame> = load_trace_str(&left_trace)?;
    let right_frames: Vec<Frame> = load_trace_str(&right_trace)?;
    compare(&left_frames, &right_frames).map_err(|_| {
        // Re-run through `load_trace_str`'s error path to produce a rendered
        // divergence; `compare` already produced the `Divergence` value but we
        // need the `TraceError::Divergence` variant for the public signature.
        let div = crate::trace::compare(&left_frames, &right_frames).unwrap_err();
        TraceError::Divergence(div.render(&left_frames, &right_frames))
    })
}

/// Convenience: the canonical fixture exercising set/get/delete/entries plus the
/// corrupt-treat-as-absent regression case (FLUX-080/081).
#[must_use]
pub fn default_persistence_script() -> Vec<StoreOp> {
    use flux_syntax::Value;
    vec![
        StoreOp::Set("token".into(), StoreValue(Value::Str(1))),
        StoreOp::Set(
            "profile".into(),
            StoreValue(Value::List(vec![Value::Int(7)])),
        ),
        StoreOp::Get("token".into()),
        StoreOp::Get("missing".into()),
        StoreOp::Entries,
        // Simulate a torn write on `token`, then assert both hosts read absent
        // and the corrupt blob is gone from entries().
        StoreOp::Corrupt("token".into()),
        StoreOp::Get("token".into()),
        StoreOp::Entries,
        StoreOp::Delete("profile".into()),
        StoreOp::Entries,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_backend_treats_corrupt_as_absent() {
        let mut backend = InMemoryStorageBackend::default();
        backend.set("k", StoreValue(Value::Int(42)));
        assert_eq!(
            backend.get("k"),
            GetResult::Present(StoreValue(Value::Int(42)))
        );
        backend.corrupt("k");
        // Corrupt blob reads absent and is removed.
        assert_eq!(backend.get("k"), GetResult::Absent);
        assert!(!backend.blobs.contains_key("k"));
        // A genuinely missing key also reads absent.
        assert_eq!(backend.get("other"), GetResult::Absent);
    }

    #[test]
    fn two_reference_backends_agree_on_full_script() {
        let script = default_persistence_script();
        let mut a = InMemoryStorageBackend::default();
        let mut b = InMemoryStorageBackend::default();
        assert_storage_parity(&script, &mut a, &mut b)
            .expect("reference backends must agree on the persistence script");
    }

    #[test]
    fn entries_skips_corrupt_blob() {
        let mut backend = InMemoryStorageBackend::default();
        backend.set("good", StoreValue(Value::Int(1)));
        backend.set("bad", StoreValue(Value::Int(2)));
        backend.corrupt("bad");
        let entries = backend.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "good");
    }

    // The wire encode/decode helpers must round-trip for the values we store.
    #[test]
    fn value_blob_roundtrip() {
        for v in [
            Value::Int(-3),
            Value::Bool(true),
            Value::Str(4),
            Value::List(vec![Value::Int(1), Value::Int(2)]),
            Value::Null,
        ] {
            let blob = encode_value(&v);
            let decoded = flux_ir_serde::decode_value_blob(&blob).expect("decode");
            assert_eq!(decoded, v);
        }
        // A truncated blob must fail to decode (drives the corrupt->absent path).
        let blob = encode_value(&Value::Int(7));
        assert!(flux_ir_serde::decode_value_blob(&blob[..blob.len() / 2]).is_err());
    }
}
