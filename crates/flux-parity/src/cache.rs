//! Image-cache eviction parity (FLUX-082).
//!
//! The production-readiness plan (§2.4) found the two hosts had diverged on
//! image-cache eviction under memory pressure: there was no cross-platform
//! assertion that both hosts evict the *same* entries in the *same* order when
//! the OS fires a low-memory warning. This module is the parity harness's
//! host-neutral model of an LRU image cache and asserts two independent backends
//! evict identically for a script of inserts/accesses/pressure signals.
//!
//! The contract modeled here (the one CI ratifies): an image cache is bounded by
//! a byte capacity; under a low-memory pressure signal it evicts the
//! least-recently-used entries first until its footprint is back under the
//! pressure budget, emitting the evicted keys **in eviction order**. Both hosts
//! must produce the same ordered eviction list. A deliberate divergence would be
//! documented and ratified separately — this harness pins the convergent
//! (LRU) behavior so silent drift is caught.

use crate::trace::{Frame, TraceError, compare, load_trace_str};

/// A host image-cache backend under test.
///
/// Two independent implementations are exercised side by side; the harness
/// asserts they agree on every eviction. The default [`LruImageCache`] is the
/// reference LRU model.
pub trait ImageCacheBackend {
    /// Inserts `bytes` of image data under `key`, marking it most-recently used.
    /// Re-inserting an existing key replaces its bytes and refreshes its recency.
    fn insert(&mut self, key: &str, bytes: usize);
    /// Marks `key` most-recently used (a cache hit). A miss is ignored.
    fn access(&mut self, key: &str);
    /// Applies a low-memory pressure signal with a `budget_bytes` ceiling and
    /// evicts least-recently-used entries until the footprint is at/under the
    /// budget. Returns the evicted keys **in eviction order**.
    fn evict_under_pressure(&mut self, budget_bytes: usize) -> Vec<String>;
}

/// The reference LRU image cache.
///
/// Tracks each key's byte size and a recency ordering (front = most recently
/// used). `insert`/`access` move a key to the front; `evict_under_pressure`
/// drops from the back (least recently used) until `total_bytes <= budget`,
/// recording the evicted keys in the order they left.
#[derive(Clone, Debug, Default)]
pub struct LruImageCache {
    sizes: std::collections::HashMap<String, usize>,
    order: Vec<String>,
    total: usize,
}

impl ImageCacheBackend for LruImageCache {
    fn insert(&mut self, key: &str, bytes: usize) {
        self.touch(key);
        if let Some(prev) = self.sizes.insert(key.to_owned(), bytes) {
            self.total = self.total.saturating_sub(prev);
        }
        self.total = self.total.saturating_add(bytes);
    }

    fn access(&mut self, key: &str) {
        self.touch(key);
    }

    fn evict_under_pressure(&mut self, budget_bytes: usize) -> Vec<String> {
        let mut evicted = Vec::new();
        while self.total > budget_bytes {
            // Evict the least-recently-used entry (back of the recency list).
            match self.order.pop() {
                None => break,
                Some(key) => {
                    if let Some(size) = self.sizes.remove(&key) {
                        self.total = self.total.saturating_sub(size);
                    }
                    evicted.push(key);
                }
            }
        }
        evicted
    }
}

impl LruImageCache {
    /// Moves `key` to the most-recently-used position (front), inserting it if
    /// absent. Used by both `insert` and `access`.
    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.insert(0, key.to_owned());
    }
}

/// One step of a cache-eviction script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheOp {
    /// `insert(key, bytes)`.
    Insert(String, usize),
    /// `access(key)` — a cache hit that refreshes recency.
    Access(String),
    /// `evictUnderPressure(budget_bytes)` — low-memory signal; emits the evicted
    /// keys in order.
    EvictUnderPressure(usize),
}

/// Runs `script` against `left` and `right` backends, returning the two JSONL
/// traces for exact diffing.
///
/// Each operation emits a canonical frame: `insert` → `{op:"insert",key,bytes}`,
/// `access` → `{op:"access",key}`, `evictUnderPressure` →
/// `{op:"evict",budget,count,order:["k0","k1",…]}` listing the evicted keys in
/// eviction order. Byte sizes and eviction order are exactly what both hosts
/// must agree on.
#[must_use]
pub fn run_cache_script(
    script: &[CacheOp],
    left: &mut dyn ImageCacheBackend,
    right: &mut dyn ImageCacheBackend,
) -> (String, String) {
    let mut left_trace = String::new();
    let mut right_trace = String::new();
    for op in script {
        emit_op(op, left, &mut left_trace);
        emit_op(op, right, &mut right_trace);
    }
    (left_trace, right_trace)
}

fn emit_op(op: &CacheOp, backend: &mut dyn ImageCacheBackend, out: &mut String) {
    let frame = match op {
        CacheOp::Insert(key, bytes) => {
            backend.insert(key, *bytes);
            format!("{{\"op\":\"insert\",\"key\":\"{key}\",\"bytes\":{bytes}}}")
        }
        CacheOp::Access(key) => {
            backend.access(key);
            format!("{{\"op\":\"access\",\"key\":\"{key}\"}}")
        }
        CacheOp::EvictUnderPressure(budget) => {
            let evicted = backend.evict_under_pressure(*budget);
            let listed = evicted
                .iter()
                .map(|k| format!("\"{k}\""))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"op\":\"evict\",\"budget\":{budget},\"count\":{},\"order\":[{}]}}",
                evicted.len(),
                listed
            )
        }
    };
    out.push_str(&frame);
    out.push('\n');
}

/// Asserts two host image-cache backends produce identical traces for `script`.
///
/// Returns the rendered [`TraceError::Divergence`] on mismatch so CI localizes a
/// cross-platform eviction regression.
///
/// # Errors
///
/// Returns [`TraceError::Divergence`] on a trace mismatch (the parity failure),
/// or [`TraceError::Json`] when a generated frame is malformed (a harness bug).
pub fn assert_cache_parity(
    script: &[CacheOp],
    left: &mut dyn ImageCacheBackend,
    right: &mut dyn ImageCacheBackend,
) -> Result<(), TraceError> {
    let (left_trace, right_trace) = run_cache_script(script, left, right);
    let left_frames: Vec<Frame> = load_trace_str(&left_trace)?;
    let right_frames: Vec<Frame> = load_trace_str(&right_trace)?;
    compare(&left_frames, &right_frames)
        .map_err(|div| TraceError::Divergence(div.render(&left_frames, &right_frames)))
}

/// The canonical fixture: fill the cache past a budget, refresh a few hot keys,
/// then fire a low-memory pressure signal and assert both hosts evict the
/// same cold keys in the same order.
#[must_use]
pub fn default_cache_script() -> Vec<CacheOp> {
    vec![
        CacheOp::Insert("hero".into(), 1024),
        CacheOp::Insert("avatar_a".into(), 512),
        CacheOp::Insert("avatar_b".into(), 512),
        CacheOp::Insert("banner".into(), 2048),
        // `hero` gets hot again — must survive the pressure signal.
        CacheOp::Access("hero".into()),
        CacheOp::Access("avatar_a".into()),
        // Low-memory signal: budget 1.5 KiB. Cold `banner` (2048) then
        // `avatar_b` (512) must evict before the hot `hero`/`avatar_a`.
        CacheOp::EvictUnderPressure(1536),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lru_evicts_cold_before_hot() {
        let mut cache = LruImageCache::default();
        cache.insert("hero", 1024);
        cache.insert("cold", 512);
        cache.access("hero");
        // Budget 1024: evict `cold` first (less recently used), keep `hero`.
        let evicted = cache.evict_under_pressure(1024);
        assert_eq!(evicted, vec!["cold".to_owned()]);
    }

    #[test]
    fn two_reference_backends_agree_on_full_script() {
        let script = default_cache_script();
        let mut a = LruImageCache::default();
        let mut b = LruImageCache::default();
        assert_cache_parity(&script, &mut a, &mut b)
            .expect("reference caches must agree on the eviction script");
    }

    #[test]
    fn reinsert_refreshes_recency_and_bytes() {
        let mut cache = LruImageCache::default();
        cache.insert("a", 100);
        cache.insert("b", 100);
        // Re-insert `a` bigger and hot: now `b` is coldest.
        cache.insert("a", 300);
        // Total 400 > budget 200: evict coldest (`b`) first, then `a`.
        let evicted = cache.evict_under_pressure(200);
        assert_eq!(evicted, vec!["b".to_owned(), "a".to_owned()]);
    }
}
