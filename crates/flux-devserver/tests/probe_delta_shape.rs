// TEMP probe: inspect Delta wire shape for several edits that touch the counter.
use flux_devserver::Pipeline;
use flux_ir_serde::Frame;
use flux_syntax::Patch;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/counter");

const BASE: &str = "component Counter {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped {count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 1 })
    }
}
";

fn label_of(pt: &Patch) -> String {
    match pt {
        Patch::Handler { id, closure } => format!(
            "Handler id={} hash={:016x} len={}",
            id, closure.hash, closure.bytecode_len
        ),
        Patch::Replace { id, .. } => format!("Replace id={}", id),
        Patch::Update { id, .. } => format!("Update id={}", id),
        Patch::Remove { id } => format!("Remove id={}", id),
        Patch::Insert { .. } => "Insert".to_string(),
        other => format!("Other({:?})", std::mem::discriminant(other)),
    }
}

fn run(name: &str, src1: &str, src2: &str) {
    let mut p = Pipeline::new(ROOT, false);
    let path = format!("{}/main.flux", ROOT);
    p.set_source(std::path::Path::new(&path), src1.to_string());
    let _ = match p.compile() {
        Ok(flux_devserver::Compiled::Init(b)) => b,
        other => panic!("init: {:?}", std::mem::discriminant(&other)),
    };
    p.set_source(std::path::Path::new(&path), src2.to_string());
    let delta_bytes = match p.compile() {
        Ok(flux_devserver::Compiled::Delta(b)) => b,
        other => {
            eprintln!("[{}] -> {:?}", name, std::mem::discriminant(&other));
            return;
        }
    };
    let delta = Frame::from_delta_bytes(&delta_bytes).expect("delta decode");
    eprintln!("=== [{}] ===", name);
    eprintln!("  patches({}):", delta.patches.len());
    for pt in &delta.patches {
        eprintln!("    {}", label_of(pt));
    }
    eprintln!("  closures: {}", delta.closures.len());
    eprintln!(
        "  signal_meta nodes: {:?}",
        delta
            .signal_meta
            .iter()
            .map(|m| m.node_id)
            .collect::<Vec<_>>()
    );
    eprintln!("  flags = {:#04x}", delta.flags);
}

#[test]
fn probe_variants() {
    // (A) handler body: count + 1 -> count + 2
    run(
        "A: handler +1->+2",
        BASE,
        &BASE.replace("count + 1", "count + 2"),
    );
    // (B) handler expression change: count + 1 -> count * 2
    run(
        "B: handler +1->*2",
        BASE,
        &BASE.replace("count + 1", "count * 2"),
    );
    // (C) init state value: 0 -> 5
    run(
        "C: state 0->5",
        BASE,
        &BASE.replace("state count: Int = 0", "state count: Int = 5"),
    );
    // (D) thunk text literal change
    run(
        "D: text literal",
        BASE,
        &BASE.replace("tapped {count} times", "pressed {count} times"),
    );
    // (E) both: handler + text
    run(
        "E: handler+text",
        BASE,
        &BASE
            .replace("count + 1", "count + 2")
            .replace("tapped", "pressed"),
    );
}
