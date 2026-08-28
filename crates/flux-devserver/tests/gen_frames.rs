// TEMP: generate init + handler-body-edit delta frames to files for Android repro.
use flux_devserver::Pipeline;
use flux_ir_serde::Frame;
use flux_syntax::Patch;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../examples/counter");
const SRC1: &str = "compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n        Text(text: \"tapped {count} times\")\n        Button(text: \"Increment\", onClick: fn() { count = count + 1 })\n    }\n\n";
const SRC2: &str = "compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n        Text(text: \"tapped {count} times\")\n        Button(text: \"Increment\", onClick: fn() { count = count + 2 })\n    }\n\n";

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
    std::fs::write("/tmp/flux_init.bin", &init).unwrap();
    let delta = {
        let mut p = Pipeline::new(ROOT, false);
        p.set_source(std::path::Path::new(&path), SRC1.to_string());
        p.compile().ok();
        p.set_source(std::path::Path::new(&path), SRC2.to_string());
        match p.compile() {
            Ok(flux_devserver::Compiled::Delta(b)) => b,
            other => panic!("delta: {:?}", std::mem::discriminant(&other)),
        }
    };
    std::fs::write("/tmp/flux_delta.bin", &delta).unwrap();
    // sanity: decode delta and count patches
    let f = Frame::from_delta_bytes(&delta).expect("decode");
    let handler_patches = f
        .patches
        .iter()
        .filter(|p| matches!(p, Patch::Handler { .. }))
        .count();
    eprintln!(
        "DELTA patches={} handler_patches={}",
        f.patches.len(),
        handler_patches
    );
    eprintln!("init bytes={} delta bytes={}", init.len(), delta.len());
}
