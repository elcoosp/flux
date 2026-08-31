// TEMP: generate init + text-edit delta frames to the Android host test
// resources, so the E2E counter / hot-reload tests run against frames the
// REAL pipeline emits (canonical prop-index-keyed thunk records).
use flux_devserver::Pipeline;
use flux_ir_serde::Frame;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../examples/counter");
// Text says "tapped {count} times".
const SRC1: &str = "compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n        Text(text: \"tapped {count} times\")\n        Button(text: \"Increment\", onPress: fn() { count = count + 1 })\n    }\n\n";
// Same, but the Text literal is edited (appended "!") — a source text edit the
// dev server ships as a Delta frame (Remove+Insert whole tree).
const SRC3: &str = "compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n        Text(text: \"tapped {count} times!\")\n        Button(text: \"Increment\", onPress: fn() { count = count + 1 })\n    }\n\n";

const INIT_OUT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtimes/android/host/src/test/resources/counter_init_frame.bin");
const DELTA_OUT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtimes/android/host/src/test/resources/counter_delta_interp.bin");

#[test]
fn gen_frames() {
    let path = format!("{}/main.flux", ROOT);
    let init = {
        let mut p = Pipeline::new(ROOT, false);
        p.set_source(std::path::Path::new(&path), SRC1.to_string());
        match p.compile() {
            Ok(flux_devserver::Compiled::Init(b)) => b,
            other => panic!("init: {:?}", std::mem::discriminant(&other)),
        }
    };
    std::fs::write(INIT_OUT, &init).unwrap();
    // Sanity: the init frame must carry a signal-meta thunk for the Text node,
    // and the pipeline must emit a canonical prop-index-keyed thunk record
    // (the contract both hosts rely on). Decoding proves the frame is valid.
    let f = Frame::from_init_bytes(&init).expect("decode init");
    assert!(
        f.signal_meta.iter().any(|m| m.thunk.is_some()),
        "init frame must carry a prop-thunk for the interpolated Text"
    );

    let delta = {
        let mut p = Pipeline::new(ROOT, false);
        p.set_source(std::path::Path::new(&path), SRC1.to_string());
        p.compile().ok();
        p.set_source(std::path::Path::new(&path), SRC3.to_string());
        match p.compile() {
            Ok(flux_devserver::Compiled::Delta(b)) => b,
            other => panic!("delta: {:?}", std::mem::discriminant(&other)),
        }
    };
    std::fs::write(DELTA_OUT, &delta).unwrap();
    let f = Frame::from_delta_bytes(&delta).expect("decode delta");
    eprintln!("DELTA patches={}", f.patches.len());
    eprintln!("init bytes={} delta bytes={}", init.len(), delta.len());
}
