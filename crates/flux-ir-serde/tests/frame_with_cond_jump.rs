//! FLUX-F46 regression: a frame whose closure bytecode contains a
//! `COND_JUMP` must decode successfully. Before the fix, the frame
//! decoder read the jump target at the wrong offset and rejected it.

use flux_ir::ClosureIR;
use flux_ir_serde::Frame;
use flux_syntax::{HandlerId, Patch, Span};

fn cond_jump_handler_body() -> Vec<u8> {
    // Hand-assemble:
    //   0: LOAD_BOOL_CONST dst=0 imm=0  (op+reg+u8)   = 3 bytes
    //   3: COND_JUMP_NOT reg=0 target=0 (op+reg+i32)  = 6 bytes (next @ 9)
    //   9: HALT                                        = 1 byte
    let mut body = Vec::new();
    body.push(flux_syntax::opcode::Opcode::LoadBoolConst as u8);
    body.push(0); // dst = r0
    body.push(0); // false
    body.push(flux_syntax::opcode::Opcode::CondJumpNot as u8);
    body.push(0); // reg = r0
    body.extend_from_slice(&0i32.to_le_bytes()); // target = 0 → next (9)
    body.push(flux_syntax::opcode::Opcode::Halt as u8);
    body
}

#[test]
fn frame_with_conditional_handler_decodes() {
    let body = cond_jump_handler_body();
    let body_for_assert = body.clone();
    let closure = ClosureIR::new(HandlerId::from(1u32), body, vec![], Span::new(0, 0, 0));

    let frame_bytes = Frame::delta(
        0,
        0,
        &[Patch::Update {
            id: flux_syntax::NodeId::from(0u32),
            props_diff: flux_syntax::PropDiff {
                changes: vec![],
                removals: vec![],
            },
        }],
        &[],
        &[closure],
        &[],
    )
    .try_to_bytes()
    .expect("encode frame");

    // Before F46, the validator would have read the wrong 4 bytes for the
    // cond-jump target and rejected with
    // `MalformedBytecode { detail: "jump-out-of-range" }`.
    let decoded = Frame::from_delta_bytes(&frame_bytes)
        .expect("frame with a conditional handler body must decode");
    assert_eq!(decoded.version, flux_ir_serde::PROTOCOL_VERSION);

    // The handler's bytecode must round-trip verbatim.
    let bodies: Vec<&Vec<u8>> = decoded.closures.iter().map(|c| &c.bytecode).collect();
    assert!(
        bodies.iter().any(|b| **b == body_for_assert),
        "the handler's bytecode must round-trip verbatim; got bodies: {:?}",
        bodies,
    );
}
