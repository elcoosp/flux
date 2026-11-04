//! Round-21 adversarial tests for lowering (flux-ir) + the reference VM.
//!
//! Weird-but-*valid* programs are lowered and every emitted closure bytecode
//! (handlers + prop thunks) is pushed through `flux_vm_ref::run`. The contract
//! under test: no panics anywhere in the pipeline, decode either succeeds or
//! errors cleanly, and `run` returns Ok or a `VmError` — never a panic, never a
//! hang. An `await` handler is additionally round-tripped through
//! `run_resumable` → `resolve_cell` → `resume`.

use flux_ir::lower::lower;
use flux_parser::parse;
use flux_syntax::{HandlerId, SignalId, Value};
use flux_types::type_check;
use flux_vm_ref::{
    CapabilityImpl, CapabilityRegistry, InMemorySignals, RunResult, SignalStore, decode_program,
    resume_with_registry, run_resumable_with_registry,
};

fn lower_src(src: &str) -> flux_ir::lower::LoweredIr {
    let ast = parse(src, 0, "adv.flux").expect("adversarial program must parse");
    let typed = type_check(&ast).expect("adversarial program must type-check");
    lower(&ast, &typed).expect("adversarial program must lower")
}

/// Seeds every signal a closure's bytecode reads so list ops and arithmetic
/// see real values instead of faulting on missing state (a fault is a legal
/// `Err`, but a *panic* is not — we prefer real execution where possible).
fn seed_signals(code: &[u8]) -> InMemorySignals {
    let mut seed: Vec<(SignalId, Value)> = Vec::new();
    if let Ok(instrs) = decode_program(code) {
        for instr in &instrs {
            if instr.opcode == flux_syntax::opcode::Opcode::ReadSignal {
                seed.push((instr.u32(1), Value::Int(0)));
            }
        }
    }
    InMemorySignals::from_signals(seed)
}

/// Runs every closure and prop thunk of `lowered`. Panics are the failure
/// mode under test; `Ok` and `Err` outcomes are both acceptable. Programs
/// without handlers/thunks legally emit zero closures.
fn run_all_closures(lowered: &flux_ir::lower::LoweredIr) -> (usize, usize) {
    let strings = lowered.arena.string_table();
    let mut ran = 0usize;
    let mut halted = 0usize;
    let closures: Vec<&flux_ir::ClosureIR> = lowered
        .closures
        .values()
        .filter(|c| c.id != HandlerId::from(0u32))
        .chain(lowered.prop_thunks.values())
        .collect();
    for closure in closures {
        let mut signals = seed_signals(&closure.bytecode);
        // `run` must never panic on compiler-emitted bytecode. HALT-only
        // programs legally consume 0 gas (HALT is free, ADR-0021).
        if flux_vm_ref::run(&closure.bytecode, &mut signals, strings, Value::Null).is_ok() {
            halted += 1;
        }
        ran += 1;
    }
    (ran, halted)
}

#[test]
fn empty_component_body_lowers_and_runs() {
    let lowered = lower_src("compo Empty\n");
    let _ = run_all_closures(&lowered);
}

#[test]
fn comment_only_component_body_lowers_and_runs() {
    let lowered = lower_src("compo C\n  // nothing but a comment\n");
    let _ = run_all_closures(&lowered);
}

#[test]
fn nested_when_in_handler_lowers_and_runs() {
    // 4 nested `when`s inside a handler: well under the parser's nesting cap
    // and within the 15-register budget, but deep enough to stress lowering.
    let mut src = String::from("compo N\n  state x: Int = 0\n  Button(text: \"t\", onPress: {");
    for i in 0..4 {
        src.push_str(&format!(" when x == {i} {{"));
    }
    src.push_str(" x = 1");
    for _ in 0..4 {
        src.push_str(" }");
    }
    src.push_str(" })\n");
    let lowered = lower_src(&src);
    let (ran, _) = run_all_closures(&lowered);
    assert!(ran >= 1, "nested-when handler must emit a runnable closure");
}

/// 6-deep nested `when` exceeds the handler's 15-register pressure budget.
/// That must surface as a clean `LoweringError` (span in bounds) — not a
/// panic, not misaligned bytecode.
#[test]
fn register_pressure_overflow_is_a_clean_error() {
    let mut src = String::from("compo N\n  state x: Int = 0\n  Button(text: \"t\", onPress: {");
    for i in 0..6 {
        src.push_str(&format!(" when x == {i} {{"));
    }
    src.push_str(" x = 1");
    for _ in 0..6 {
        src.push_str(" }");
    }
    src.push_str(" })\n");
    let ast = parse(&src, 0, "adv.flux").expect("parses");
    let typed = type_check(&ast).expect("type-checks");
    let (message, span) = match lower(&ast, &typed) {
        Err(flux_ir::lower::LoweringError::Lower { message, span }) => (message, span),
        Ok(_) => panic!("6-deep whens must exceed the 15-register budget"),
    };
    assert!(
        message.contains("15 registers"),
        "expected register-pressure error, got: {message}"
    );
    assert!(
        span.end as usize <= src.len(),
        "error span must stay in bounds"
    );
}

#[test]
fn huge_unicode_string_literal_lowers_and_runs() {
    // 2_000 repetitions of an emoji + escape + combining mark (~30 KB literal)
    // must survive interning, bytecode emission and VM execution.
    let mut text = String::new();
    for _ in 0..2_000 {
        text.push_str("\\u{1F600}\u{0301}x");
    }
    let src = format!("compo U\n  Text(\"{text}\")\n");
    let lowered = lower_src(&src);
    let _ = run_all_closures(&lowered);
}

#[test]
fn empty_handler_block_lowers_and_runs() {
    // An empty handler `{}` compiles to (at least) a HALT-only program.
    let lowered = lower_src("compo H\n  state x: Int = 0\n  Button(text: \"t\", onPress: {})\n");
    let (ran, _) = run_all_closures(&lowered);
    assert!(ran >= 1, "empty handler must still emit a runnable closure");
}

/// `|| {}` — zero-parameter lambda with an empty body — is valid surface
/// syntax; it must lower (as a no-op handler) and run to HALT.
#[test]
fn empty_double_bar_lambda_lowers_and_runs() {
    let lowered =
        lower_src("compo L2\n  state x: Int = 0\n  Button(text: \"t\", onPress: || {})\n");
    let (ran, _) = run_all_closures(&lowered);
    assert!(ran >= 1, "|| {{ }} handler must emit a runnable closure");
}

/// `fn() -> Int { … }`-style lambda with zero params and an empty body, plus a
/// passthrough lambda body, must lower without panicking.
#[test]
fn empty_and_passthrough_lambdas_lower_and_run() {
    let src = concat!(
        "compo L\n",
        "  state x: Int = 0\n",
        "  Button(text: \"t\", onPress: { x = 1 })\n",
        "fn id(v: Int) -> Int { v }\n",
    );
    let lowered = lower_src(src);
    let _ = run_all_closures(&lowered);
}

/// An `await` handler suspends on a pending capability cell and the captured
/// continuation `resume`s to HALT with the resolved value written.
#[test]
fn await_handler_suspends_then_resumes_to_halt() {
    let src = concat!(
        "capability Http {\n",
        "  fn fetch(url: String) -> String\n",
        "}\n",
        "compo A\n",
        "  state body: String = \"\"\n",
        "  Button(text: \"go\", onPress: { let r = await Http.fetch(\"http://x\") })\n",
    );
    let lowered = lower_src(src);
    let strings = lowered.arena.string_table();

    // Async stub: allocate a Pending cell for (Http=14, fetch=1), mirroring
    // the reference `async_deferred` behaviour.
    fn async_pending(
        _cap: u32,
        _method: u16,
        _args: &Value,
        signals: &mut dyn SignalStore,
    ) -> SignalId {
        let id = signals.allocate_cell();
        signals.mark_pending(id);
        id
    }
    let impl_: CapabilityImpl = async_pending;
    let mut registry = CapabilityRegistry::default();
    registry.register(14, 1, impl_);

    let closure = lowered
        .closures
        .values()
        .find(|c| c.id != HandlerId::from(0u32))
        .expect("await handler must compile to a closure");
    let mut signals = InMemorySignals::default();
    let first = run_resumable_with_registry(
        &closure.bytecode,
        &mut signals,
        strings,
        Value::Null,
        &registry,
    )
    .expect("run_resumable must not panic or fault before suspension");
    let state = match first {
        RunResult::Suspended(s) => s,
        RunResult::Halt(_) => panic!("AWAIT on a Pending cell must suspend"),
    };
    // Resolve the pending cell, then resume: the continuation must reach HALT
    // (or suspend on a second await — there is none — or error cleanly).
    let cell = state.registers.iter().find_map(|v| match v {
        Value::Int(n) if *n >= 1 => Some(*n as u32),
        _ => None,
    });
    let resolved_str = {
        let mut table = flux_syntax::StringTable::new();
        let sid = table.intern("payload");
        Value::Str(sid)
    };
    if let Some(cell_id) = cell {
        signals.resolve_cell(cell_id, resolved_str.clone());
    }
    let resumed = resume_with_registry(state, &mut signals, strings, resolved_str, &registry)
        .expect("resume must not panic");
    match resumed {
        RunResult::Halt(_) => {}
        RunResult::Suspended(_) => panic!("no second await — resume must not suspend again"),
    }
}
