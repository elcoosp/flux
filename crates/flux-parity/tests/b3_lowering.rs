//! FLUX-063 regression gate: every Appendix B.3 example must lower through
//! `flux-ir` without an `unsupported handler operand/expression` error. This
//! pins the 6 examples the issue named (b32, b33, b35, b36, b38, b310) — plus
//! the other four — so the lowering gap cannot silently reopen.

use flux_ir::lower::lower;
use flux_parser::parse;
use flux_types::type_check;

/// The ten Appendix B.3 grammar examples, reproduced from `docs/spec/
/// mlp-appendices.md`. Kept inline (rather than imported) so this regression
/// gate does not depend on the harness's private `sources` module.
const B3_EXAMPLES: &[(&str, &str)] = &[
    (
        "b31_simple",
        "compo HelloWorld\n  state count: Int = 0\n\n  Column(gap: 12) {\n    Text(\"Count: {count}\")\n    Button(text: \"Increment\", onClick: {\n      count = count + 1\n    })\n}\n",
    ),
    (
        "b32_generic",
        "trait Numeric[T] {\n  fn zero() -> T\n  fn one() -> T\n  fn +(a: T, b: T) -> T\n  fn -(a: T, b: T) -> T\n}\n\ncompo Counter[T: Numeric]\n  state count: T = Numeric.zero()\n\n  Column(gap: 8) {\n    Text(\"Count: {count}\")\n    Button(text: \"+\", onClick: { count = count + Numeric.one() })\n    Button(text: \"-\", onClick: { count = count - Numeric.one() })\n}\n",
    ),
    (
        "b33_adt",
        "type Shape =\n  | Circle(Float)\n  | Rectangle(Float, Float)\n  | Triangle(Float, Float, Float)\n\nfn area(shape: Shape) -> Float {\n  match shape {\n    Circle(r) => 3.14159 * r * r\n    Rectangle(w, h) => w * h\n    Triangle(b, h, _) => 0.5 * b * h\n  }\n}\n\ncompo ShapeDisplay\n  state shape: Shape = Circle(5.0)\n\n  Column {\n    Text(\"Area: {area(shape)}\")\n    Button(text: \"Make Square\", onClick: {\n      shape = Rectangle(4.0, 4.0)\n    })\n}\n",
    ),
    (
        "b34_lifecycle",
        "compo Chat\n  state messages: List[String] = []\n  let socket = createRef[WebSocket]()\n\n  onMount {\n    socket.set(WebSocket.connect(\"ws://localhost:8080\"))\n    socket.get().on_message = fn(msg: String) {\n      batch {\n        messages = messages + [msg]\n    }\n  }\n  }\n\n  onCleanup {\n    socket.get().close()\n  }\n\n  Column {\n    ForEach(messages, key: fn(m, i) { i }) { msg =>\n      Text(msg)\n    }\n  }\n\n",
    ),
    (
        "b35_navigation",
        "compo App\n  state route: String = \"home\"\n\n  Router {\n    Screen(\"home\") { Home() }\n    Screen(\"profile\") { Profile() }\n    Screen(\"settings\") { Settings() }\n}\n\ncompo Home\n  let router = useContext(RouterContext)\n\n  Column(gap: 16) {\n    Text(\"Home\")\n    Button(text: \"Open Profile\", onClick: {\n      router.navigate(\"profile\")\n    })\n}\n\ncompo Profile\n  Column { Text(\"Profile\") }\n\ncompo Settings\n  Column { Text(\"Settings\") }\n",
    ),
    (
        "b36_async",
        "compo UserList\n  let (users, { refetch }) = resource(fn {\n    Api.fetch(\"/users\")\n  })\n\n  Column {\n    when users.is_loading {\n      Text(\"Loading...\")\n    }\n    otherwise {\n      ForEach(users.value, key: fn(u) { u.id }) { user =>\n        Text(\"{user.name}\")\n      }\n    }\n    Button(text: \"Refresh\", onClick: { refetch() })\n  }\n\n",
    ),
    (
        "b37_pure",
        "@pure\ncompo Avatar(url: String, size: Float)\n  Image(url) {\n    width: size,\n    height: size,\n    cornerRadius: size / 2\n}\n\ncompo Profile\n  state avatarUrl: String = \"https://example.com/me.png\"\n\n  Column {\n    Avatar(url: avatarUrl, size: 80)\n    Text(\"Profile\")\n}\n",
    ),
    (
        "b38_platform",
        "compo PlatformButton\n  if platform() == \"ios\" {\n    CupertinoButton(text: \"Tap\", onClick: { 1 })\n  } else {\n    MaterialButton(text: \"Tap\", onClick: { 1 })\n  }\n\n",
    ),
    (
        "b39_capability",
        "capability Camera {\n  fn capture() -> Data\n  fn startPreview() -> Unit\n  fn stopPreview() -> Unit\n}\n\ncapability Storage {\n  fn set(key: String, value: Data) -> Unit\n  fn get(key: String) -> Option[Data]\n  fn delete(key: String) -> Unit\n}\n",
    ),
    (
        "b310_refs",
        "compo LoginForm\n  let emailRef = createRef[TextField]()\n  let passwordRef = createRef[TextField]()\n\n  onMount {\n    emailRef.focus()\n  }\n\n  Column(gap: 12) {\n    TextField(ref: emailRef, placeholder: \"Email\")\n    TextField(ref: passwordRef, placeholder: \"Password\")\n    Button(text: \"Submit\", onClick: {\n      let email = emailRef.text()\n      let password = passwordRef.text()\n      Auth.login(email, password)\n    })\n  }\n\n",
    ),
];

#[test]
fn all_b3_examples_lower_without_unsupported_error() {
    let mut lowered_ok = 0usize;
    let mut failures = Vec::new();
    for (name, src) in B3_EXAMPLES {
        let ast = parse(src, 1, "b3.flux").unwrap_or_else(|e| panic!("{name}: parse failed: {e}"));
        let typed = type_check(&ast).unwrap_or_else(|e| panic!("{name}: type-check failed: {e:?}"));
        match lower(&ast, &typed) {
            Ok(_) => lowered_ok += 1,
            Err(e) => failures.push(format!("{name}: {e:?}")),
        }
    }
    assert_eq!(
        lowered_ok,
        B3_EXAMPLES.len(),
        "B.3 examples that failed flux-ir lowering: {}",
        failures.join("\n")
    );
}
