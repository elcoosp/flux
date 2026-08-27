// TEMP: dump ALL patches (not just Handler) for a handler-body edit.
use flux_devserver::Pipeline;
use flux_ir_serde::Frame;
use flux_syntax::Patch;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/counter");
const SRC1: &str = "component Counter {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped {count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 1 })
    }
}
";
const SRC2: &str = "component Counter {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped {count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 2 })
    }
}
";

#[test]
fn dump_all_patches() {
    let mut p = Pipeline::new(ROOT, false);
    let path = format!("{}/main.flux", ROOT);
    p.set_source(std::path::Path::new(&path), SRC1.to_string());
    let _init = p.compile().expect("init");
    p.set_source(std::path::Path::new(&path), SRC2.to_string());
    let bytes = match p.compile() {
        Ok(flux_devserver::Compiled::Delta(b)) => b,
        other => panic!("expected Delta, got {:?}", std::mem::discriminant(&other)),
    };
    let f = Frame::from_delta_bytes(&bytes).expect("decode");
    eprintln!("== DELTA signal_meta node ids ==");
    for m in &f.signal_meta {
        eprintln!("  node_id={} has_thunk={}", m.node_id, m.thunk.is_some());
    }
    // Handlers now travel inside the per-node closure blob (DeltaFrame no longer
    // carries a top-level `handlers` table); they are surfaced as `Patch::Handler`
    // entries below, which already print handler id + bytecode length.
    eprintln!("== DELTA patches ({} total) ==", f.patches.len());
    let init_bytes = match Pipeline::new(ROOT, false) {
        mut pp => {
            pp.set_source(std::path::Path::new(&path), SRC1.to_string());
            match pp.compile() {
                Ok(flux_devserver::Compiled::Init(b)) => b,
                _ => panic!("init"),
            }
        }
    };
    let fi = Frame::from_init_bytes(&init_bytes).expect("init decode");
    eprintln!("== INIT node ids ==");
    eprintln!("  root={}", fi.root.id);
    for n in &fi.extra_nodes {
        eprintln!("  extra={}", n.id);
    }
    eprintln!("== DELTA patches ({} total) ==", f.patches.len());
    for (i, pt) in f.patches.iter().enumerate() {
        match pt {
            Patch::Handler { id, closure } => {
                eprintln!("[{}] Handler id={} len={}", i, id, closure.bytecode_len)
            }
            Patch::Update { id, .. } => eprintln!("[{}] Update id={}", i, id),
            Patch::Replace { id, .. } => eprintln!("[{}] Replace id={}", i, id),
            Patch::Insert { .. } => eprintln!("[{}] Insert", i),
            Patch::Remove { id } => eprintln!("[{}] Remove id={}", i, id),
            other => eprintln!("[{}] {:?}", i, other),
        }
    }
}
