//! Round-10 verification: `LatencyMs`'s custom `Deserialize` must reject
//! negative / NaN / infinite latencies so a malformed `PerfRecord` JSON cannot
//! reach `LatencyMs::from_raw` (which panics) or `partial_cmp` (which expects
//! no NaN). `flux-devtools-ui::DevToolsState::ingest_perf_record` is the
//! consumer; a bad JSON string must be dropped, never panic the app.

use flux_perf_harness::MetricRecord;

#[test]
fn from_json_rejects_negative_latency() {
    let json = r#"{
      "scenario":"loopback-e2e",
      "kind":"patch-round-trip",
      "tree_size":50,
      "samples":[{"latency":-1.0,"size":null}]
    }"#;
    let parsed = MetricRecord::from_json(json);
    assert!(
        parsed.is_err(),
        "negative latency must be rejected, got {parsed:?}"
    );
}

#[test]
fn from_json_rejects_nan_latency() {
    // serde_json has no NaN literal, but `null` for a f64 field is rejected by
    // the custom Deserialize anyway. Cover the path with a JSON `null` where a
    // number is required.
    let json = r#"{
      "scenario":"loopback-e2e",
      "kind":"patch-round-trip",
      "tree_size":50,
      "samples":[{"latency":null,"size":null}]
    }"#;
    assert!(MetricRecord::from_json(json).is_err(), "null latency must be rejected");
}

#[test]
fn from_json_rejects_infinite_latency_string() {
    // `1e400` parses to `f64::INFINITY` in some deserializers.
    let json = r#"{
      "scenario":"loopback-e2e",
      "kind":"patch-round-trip",
      "tree_size":50,
      "samples":[{"latency":1e400,"size":null}]
    }"#;
    assert!(
        MetricRecord::from_json(json).is_err(),
        "infinite latency must be rejected",
    );
}

#[test]
fn from_json_accepts_zero_and_positive() {
    let json = r#"{
      "scenario":"loopback-e2e",
      "kind":"patch-round-trip",
      "tree_size":50,
      "samples":[{"latency":0.0,"size":null},{"latency":3.5,"size":100}]
    }"#;
    let rec = MetricRecord::from_json(json).expect("well-formed JSON");
    assert_eq!(rec.samples.len(), 2);
}
