//! FLUX-082 LANE-PARITY integration tests: persistence, image-cache eviction,
//! and error-frame (wire-version) rejection.
//!
//! Each subsystem is driven through two independent host models (the reference
//! backend plus a second reference instance — the two stand-ins for the iOS and
//! Android hosts) and asserted identical. The corrupt-entry persistence case is
//! the regression test for FLUX-080/FLUX-081 (both hosts treat corrupt as
//! absent); the error-frame case is the cross-decoder reject contract (FLUX-083).

use flux_parity::{
    InMemoryStorageBackend, LruImageCache, StorageBackend, assert_cache_parity,
    assert_error_frame_parity, assert_storage_parity, default_cache_script, default_corpus,
    default_persistence_script,
};

/// Persistence: two reference backends (stand-ins for iOS + Android) must agree
/// on the full set/get/delete/entries script, including the corrupt-treat-as-
/// absent contract (FLUX-080/081 regression).
#[test]
fn persistence_reference_backends_agree() {
    let script = default_persistence_script();
    let mut ios = InMemoryStorageBackend::default();
    let mut android = InMemoryStorageBackend::default();
    assert_storage_parity(&script, &mut ios, &mut android)
        .expect("iOS and Android storage backends must agree (FLUX-080/081)");
}

/// Cache: two reference backends must evict the same keys in the same order
/// under a low-memory pressure signal.
#[test]
fn cache_reference_backends_agree() {
    let script = default_cache_script();
    let mut ios = LruImageCache::default();
    let mut android = LruImageCache::default();
    assert_cache_parity(&script, &mut ios, &mut android)
        .expect("iOS and Android image caches must evict identically (FLUX-082)");
}

/// Error-frame: the Rust reference decoder and the modeled host decoder must
/// agree on every frame in the corpus (valid accepted; version/magic/truncated
/// rejected fail-closed) — the cross-decoder reject contract (FLUX-083).
#[test]
fn error_frame_reference_and_host_agree() {
    assert_error_frame_parity().expect("reference and host decoders must agree on the corpus");
}

/// Persistence: the same key corrupted on both backends reads back as `absent`
/// and the corrupt blob is gone from `entries()` — the exact FLUX-080/081
/// contract the plan required the harness to be able to assert.
#[test]
fn corrupt_entry_is_absent_on_both_hosts() {
    let mut ios = InMemoryStorageBackend::default();
    let mut android = InMemoryStorageBackend::default();
    for backend in [&mut ios, &mut android] {
        backend.set("token", flux_parity::StoreValue(flux_syntax::Value::Str(1)));
        backend.corrupt("token");
        assert_eq!(
            backend.get("token"),
            flux_parity::GetResult::Absent,
            "corrupt entry must read absent"
        );
        assert!(backend.entries().iter().all(|(k, _)| k != "token"));
    }
}

/// Error-frame: every malformed variant in the corpus is rejected by BOTH the
/// reference decoder and a second modeled host decoder, and the two agree.
#[test]
fn every_malformed_frame_rejected_by_both() {
    for frame in default_corpus() {
        if frame.label.starts_with("valid") {
            continue;
        }
        let ref_outcome = flux_parity::ReferenceDecoder::decode(&frame.bytes);
        let host_outcome = flux_parity::HostDecoder::decode(&frame.bytes);
        assert_eq!(
            ref_outcome, host_outcome,
            "frame `{}`: reference and host decoders must agree on rejection",
            frame.label
        );
        assert!(
            matches!(host_outcome, flux_parity::Rejection::Rejected(_)),
            "frame `{}` must be rejected fail-closed",
            frame.label
        );
    }
}

/// Cache: a second, homogeneously-built cache instance (a different host build)
/// must produce a trace identical to the first for a stress script.
#[test]
fn cache_eviction_order_is_deterministic_across_instances() {
    // A deeper stress script than the default fixture to shake out any
    // order-dependent drift between two independent instances.
    use flux_parity::CacheOp;
    let script = vec![
        CacheOp::Insert("a".into(), 100),
        CacheOp::Insert("b".into(), 200),
        CacheOp::Insert("c".into(), 300),
        CacheOp::Insert("d".into(), 400),
        CacheOp::Access("a".into()),
        CacheOp::Access("c".into()),
        CacheOp::EvictUnderPressure(500),
        CacheOp::Insert("e".into(), 250),
        CacheOp::Access("b".into()),
        CacheOp::EvictUnderPressure(400),
    ];
    let mut h1 = LruImageCache::default();
    let mut h2 = LruImageCache::default();
    assert_cache_parity(&script, &mut h1, &mut h2)
        .expect("two cache instances must evict identically");
}
