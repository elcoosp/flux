// Regression test: a handler-body edit (count + 1 -> count + 2) must ship a
// `Patch::Handler` carrying the updated closure bytecode, so hosts re-register
// the new body instead of keeping the stale init-time one.
//
// Root cause this guards: lowering never populated `IRArena`'s closure table
// (only the serialized `closures` Vec), so the differ's `IRArena::closure()`
// lookup returned `None` for every handler id and `emit_handler` emitted
// nothing — a handler-body edit produced an empty delta on both iOS and
// Android. See FLUX-014.
use flux_devserver::Pipeline;
use flux_ir_serde::Frame;
use flux_syntax::Patch;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/counter");

const SRC1: &str = "compo Counter
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped {count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 1 })
    }

";

const SRC2: &str = "compo Counter
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped {count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 2 })
    }

";

#[test]
fn handler_body_edit_emits_patch_with_updated_bytecode() {
    let mut p = Pipeline::new(ROOT, false);
    let path = format!("{}/main.flux", ROOT);
    p.set_source(std::path::Path::new(&path), SRC1.to_string());
    match p.compile() {
        Ok(flux_devserver::Compiled::Init(_)) => {}
        other => panic!(
            "first compile should be Init, got {:?}",
            std::mem::discriminant(&other)
        ),
    }

    p.set_source(std::path::Path::new(&path), SRC2.to_string());
    let bytes = match p.compile() {
        Ok(flux_devserver::Compiled::Delta(b)) => b,
        other => panic!(
            "second compile should be Delta, got {:?}",
            std::mem::discriminant(&other)
        ),
    };

    let delta = Frame::from_delta_bytes(&bytes).expect("delta decode");
    let handler_patches: Vec<_> = delta
        .patches
        .iter()
        .filter_map(|pt| match pt {
            Patch::Handler { id, closure } => Some((*id, closure.bytecode_len)),
            _ => None,
        })
        .collect();
    assert!(
        !handler_patches.is_empty(),
        "handler-body edit must emit at least one Patch::Handler (got {})",
        delta.patches.len()
    );

    // The new closure body must carry the +2 immediate. The onClick bytecode
    // is `... b0 02 02 00 00 00 ...` where bytes [8..12] are the little-endian
    // addend (0x02 = +2). A stale +1 body would read 0x01 there.
    let updated = delta
        .closures
        .iter()
        .find(|c| c.bytecode.len() == 27)
        .expect("onClick closure present in delta");
    assert_eq!(
        updated.bytecode[8..12],
        [0x02, 0x00, 0x00, 0x00],
        "onClick bytecode should increment by 2, found {:02x?}",
        &updated.bytecode[8..12]
    );
}
