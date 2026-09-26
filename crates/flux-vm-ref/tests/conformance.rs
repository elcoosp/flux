//! Conformance tests: the reference VM must agree with every golden ISA vector
//! under `/tests/isa-vectors/`. These are the behavioral contract (FLUX-002) that
//! the Swift and Kotlin runtimes will also be checked against.

use std::collections::BTreeMap;
use std::path::PathBuf;

use flux_syntax::{PropIdx, SignalId, StringId, StringTable, Value};
use flux_vm_ref::{
    CapabilityRegistry, InMemorySignals, SignalStore, VmErrorKind, run, run_with_registry,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct VecValue {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum ExpectedError {
    GasExhausted,
    MemoryExhausted,
    IndexOutOfBounds,
    NullDereference,
    InvalidDispatch,
    TypeMismatch,
    DivByZero,
}

impl ExpectedError {
    fn kind(&self) -> VmErrorKind {
        match self {
            Self::GasExhausted => VmErrorKind::GasExhausted,
            Self::MemoryExhausted => VmErrorKind::MemoryExhausted,
            Self::IndexOutOfBounds => VmErrorKind::IndexOutOfBounds,
            Self::NullDereference => VmErrorKind::NullDereference,
            Self::InvalidDispatch => VmErrorKind::InvalidDispatch,
            Self::TypeMismatch => VmErrorKind::TypeMismatch,
            Self::DivByZero => VmErrorKind::DivByZero,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Vector {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    description: String,
    bytecode_hex: String,
    initial_signals: Vec<SignalSeed>,
    #[serde(default)]
    payload: Option<VecValue>,
    expected_signals: Vec<SignalSeed>,
    expected_registers: BTreeMap<String, VecValue>,
    #[serde(default)]
    expected_error: Option<ExpectedError>,
    expected_gas_used: u32,
    #[serde(default)]
    strings: Vec<StringSeed>,
    #[serde(default)]
    expected_output: Option<VecValue>,
}

#[derive(Debug, Deserialize)]
struct SignalSeed {
    id: u32,
    value: VecValue,
}

/// A string interning entry: `id` is the `StringId` operand the bytecode
/// references, `text` is the resolved string content for `StrLen` / `StrEq`
/// / `StrConcat` evaluation.
#[derive(Debug, Clone, Deserialize)]
struct StringSeed {
    id: u32,
    text: String,
}

fn to_value(v: &VecValue) -> Value {
    match v.ty.as_str() {
        "Int" => Value::Int(v.value.as_ref().and_then(|x| x.as_i64()).unwrap_or(0)),
        "Float" => Value::Float(v.value.as_ref().and_then(parse_float).unwrap_or(0.0)),
        "Bool" => Value::Bool(
            v.value
                .as_ref()
                .and_then(|x| x.as_bool().or_else(|| x.as_i64().map(|n| n != 0)))
                .unwrap_or(false),
        ),
        "Str" => Value::Str(v.value.as_ref().and_then(|x| x.as_u64()).unwrap_or(0) as StringId),
        "Null" => Value::Null,
        "List" => {
            let items = v
                .value
                .as_ref()
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|e| e.as_object().map(|_| ())).count());
            let _ = items;
            let arr = v
                .value
                .as_ref()
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            Value::List(
                arr.iter()
                    .map(|e| to_value(&serde_json::from_value(e.clone()).unwrap()))
                    .collect(),
            )
        }
        "Record" => {
            let arr = v
                .value
                .as_ref()
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            Value::Record(
                arr.iter()
                    .enumerate()
                    .map(|(i, e)| {
                        (
                            PropIdx::try_from(i).unwrap_or(0),
                            to_value(&serde_json::from_value(e.clone()).unwrap()),
                        )
                    })
                    .collect(),
            )
        }
        other => panic!("unknown value type tag: {other}"),
    }
}

fn parse_float(v: &serde_json::Value) -> Option<f64> {
    if let Some(f) = v.as_f64() {
        return Some(f);
    }
    v.as_str().and_then(|s| match s {
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        "nan" => Some(f64::NAN),
        _ => None,
    })
}

fn vector_dir() -> PathBuf {
    // Cargo runs the test with cwd = crate root (crates/flux-vm-ref).
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("..");
    p.push("..");
    p.push("tests");
    p.push("isa-vectors");
    p
}

fn load_vectors() -> Vec<Vector> {
    let dir = vector_dir();
    assert!(dir.exists(), "isa-vectors dir not found at {dir:?}");
    let mut out: Vec<Vector> = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // Skip non-conformance vector files (e.g. foreach_ids.json,
        // json_object.json) that share this directory but have a different
        // schema — only files with a `bytecode_hex` field are VM conformance
        // vectors.
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        if parsed.get("bytecode_hex").is_none() {
            continue;
        }
        let v: Vector = serde_json::from_value(parsed).unwrap();
        out.push(v);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    assert!(!out.is_empty(), "no vectors loaded from {dir:?}");
    out
}

fn approx_eq(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    if a.is_infinite() || b.is_infinite() {
        return a == b; // +inf == +inf, -inf == -inf; signs differ => false
    }
    (a - b).abs() < 1e-9
}

fn value_matches(actual: &Value, expected: &VecValue) -> bool {
    let exp = to_value(expected);
    match (actual, &exp) {
        (Value::Float(a), Value::Float(b)) => approx_eq(*a, *b),
        other => other.0 == other.1,
    }
}

#[test]
fn all_isa_vectors_pass() {
    let vectors = load_vectors();
    let mut passed = 0usize;
    let mut failed = Vec::new();
    for v in &vectors {
        let bytecode = hex::decode(&v.bytecode_hex).expect("valid hex in bytecode_hex");
        let mut signals = InMemorySignals::from_signals(
            v.initial_signals.iter().map(|s| (s.id, to_value(&s.value))),
        );
        let payload = v.payload.as_ref().map(to_value).unwrap_or(Value::Null);

        // Build the string interning table the VM uses to resolve StrLen (Appendix E §E.5).
        // Seeds are sorted by id so `intern` — which assigns dense ids from 0 in
        // insertion order — reproduces the vector's intended StringId mapping.
        let mut sorted_strings = v.strings.clone();
        sorted_strings.sort_by_key(|s| s.id);
        let mut strings = StringTable::new();
        for seed in &sorted_strings {
            let actual = strings.intern(&seed.text);
            assert_eq!(
                actual, seed.id,
                "{}: string seed id {} != intern-assigned {} for text {:?}",
                v.name, seed.id, actual, seed.text
            );
        }

        match &v.expected_error {
            Some(err) => {
                let got = run(&bytecode, &mut signals, &strings, payload);
                match got {
                    Err(e) if e.kind == err.kind() => {}
                    Err(e) => failed.push(format!(
                        "{}: expected error {:?} got {:?}",
                        v.name, err, e.kind
                    )),
                    Ok(_) => failed.push(format!(
                        "{}: expected error {:?} but succeeded",
                        v.name, err
                    )),
                }
            }
            None => {
                let out = match run(&bytecode, &mut signals, &strings, payload) {
                    Ok(o) => o,
                    Err(e) => {
                        failed.push(format!("{}: unexpected error {:?}", v.name, e.kind));
                        continue;
                    }
                };
                if out.gas_used != v.expected_gas_used {
                    failed.push(format!(
                        "{}: gas {} != expected {}",
                        v.name, out.gas_used, v.expected_gas_used
                    ));
                }
                for sig in &v.expected_signals {
                    let got = signals.read(sig.id);
                    match got {
                        Some(g) if value_matches(&g, &sig.value) => {}
                        other => failed.push(format!(
                            "{}: signal {} mismatch: {:?}",
                            v.name, sig.id, other
                        )),
                    }
                }
                for (name, exp) in &v.expected_registers {
                    let idx: usize = name[1..].parse().expect("register name rN");
                    let got = &out.registers[idx];
                    if !value_matches(got, exp) {
                        failed.push(format!("{}: register {name} mismatch: {got:?}", v.name));
                    }
                }
                if let Some(exp) = &v.expected_output {
                    if !value_matches(&out.registers[0], exp) {
                        failed.push(format!(
                            "{}: r0 mismatch: {:?} != {:?}",
                            v.name,
                            out.registers[0],
                            to_value(exp)
                        ));
                    }
                }
            }
        }
        passed += 1;
    }
    assert!(
        failed.is_empty(),
        "{} of {} vectors FAILED:\n{}",
        failed.len(),
        vectors.len(),
        failed.join("\n")
    );
    eprintln!("conformance: {passed}/{} vectors passed", vectors.len());
}

#[test]
fn custom_registry_injects_real_capability_fake() {
    // T-335.2: a conformance test that injects a real capability fake via
    // `run_with_registry`, proving CALL_CAP dispatches to a caller-supplied
    // registry rather than the hardcoded parity stubs.
    //
    // Bytecode:
    //   LOAD_INT_CONST r0, 77   ; B0 00 <77 as i64 LE>
    //   CALL_CAP  r1, cap=42, method=1, args=r0  ; 90 01 <42 u32 LE> <1 u16 LE> 00
    //   HALT                     ; 00
    let bytecode = vec![
        0xB0, 0x00, 0x4D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LOAD_INT_CONST r0, 77
        0x90, 0x01, 0x2A, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x00, // CALL_CAP r1, cap=42, m=1, args=r0
        0x00, // HALT
    ];

    // Real fake: cap 42, method 1 — writes the incoming arg to signal 200
    // and returns 200 as the result-cell id.
    fn fake_cap(
        _cap_id: u32,
        _method_id: u16,
        args: &Value,
        signals: &mut dyn SignalStore,
    ) -> SignalId {
        signals.write(200, args.clone());
        200
    }

    let mut registry = CapabilityRegistry::new();
    registry.register(42, 1, fake_cap);

    let mut signals = InMemorySignals::default();
    let out = run_with_registry(
        &bytecode,
        &mut signals,
        &StringTable::new(),
        Value::Null,
        &registry,
    )
    .expect("CALL_CAP with injected registry must run");

    // The result-cell id (200) lands in r1.
    assert_eq!(out.registers[1], Value::Int(200));
    // The fake wrote the argument (77) into signal 200.
    assert_eq!(signals.read(200), Some(Value::Int(77)));
    // Gas: LOAD_INT_CONST + CALL_CAP = 2 (HALT is free).
    assert_eq!(out.gas_used, 2);
}
