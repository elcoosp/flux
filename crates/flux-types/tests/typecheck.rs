//! Integration tests for the Flux bidirectional type checker (FLUX-012).
//!
//! These tests drive the public [`flux_types::type_check`] entry point from
//! real `.flux` source parsed by [`flux_parser::parse`], and assert:
//!
//! * the 10 Appendix B.3 grammar examples type-check,
//! * type mismatches are caught with precise spans,
//! * generic instantiations are recorded (e.g. `Counter[Int]` and
//!   `Counter[Float]` both detected),
//! * non-exhaustive `match` is rejected.

use flux_parser::parse;
use flux_types::{GenericInstantiation, type_check};

/// Parse `source` into an `Ast` with a fixed file id/path.
fn ast_of(source: &str) -> flux_parser::Ast {
    parse(source, 1, "test.flux").expect("fixture must parse")
}

/// Type-check `source`; unwraps on parse or type error so callers can assert
/// success, or use [`check_err`] to assert failure.
fn check_ok(source: &str) {
    let ast = ast_of(source);
    type_check(&ast).unwrap_or_else(|e| panic!("expected well-typed, got: {e:?}"));
}

/// Type-check `source` and return the resulting error (panics if it succeeds).
fn check_err(source: &str) -> flux_types::TypeError {
    let ast = ast_of(source);
    type_check(&ast).expect_err("expected a type error")
}

mod appendix_b3 {
    use super::*;

    #[test]
    fn b3_1_simple_component() {
        check_ok(
            "compo HelloWorld\n  state count: Int = 0\n\n  Column(gap: 12) {\n    Text(\"Count: {count}\")\n    Button(text: \"Increment\", onClick: {\n      count = count + 1\n    })\n  }\n\n",
        );
    }

    #[test]
    fn b3_2_generic_component_trait_bound() {
        check_ok(
            "trait Numeric[T] {\n  fn zero() -> T\n  fn one() -> T\n  fn +(a: T, b: T) -> T\n  fn -(a: T, b: T) -> T\n}\n\ncompo Counter[T: Numeric]\n  state count: T = Numeric.zero()\n\n  Column(gap: 8) {\n    Text(\"Count: {count}\")\n    Button(text: \"+\", onClick: { count = count + Numeric.one() })\n    Button(text: \"−\", onClick: { count = count - Numeric.one() })\n  }\n\n",
        );
    }

    #[test]
    fn b3_3_algebraic_data_type_and_match() {
        check_ok(
            "type Shape =\n  | Circle(Float)\n  | Rectangle(Float, Float)\n  | Triangle(Float, Float, Float)\n\nfn area(shape: Shape) -> Float {\n  match shape {\n    Circle(r) => 3.14159 * r * r\n    Rectangle(w, h) => w * h\n    Triangle(b, h, _) => 0.5 * b * h\n  }\n}\n\ncompo ShapeDisplay\n  state shape: Shape = Circle(5.0)\n\n  Column {\n    Text(\"Area: {area(shape)}\")\n    Button(text: \"Make Square\", onClick: {\n      shape = Rectangle(4.0, 4.0)\n    })\n  }\n\n",
        );
    }

    #[test]
    fn b3_4_lifecycle_effects_cleanup() {
        check_ok(
            "compo Chat\n  state messages: List[String] = []\n  let socket = createRef[WebSocket]()\n\n  onMount {\n    socket.set(WebSocket.connect(\"ws://localhost:8080\"))\n    socket.get().on_message = fn(msg: String) {\n      batch {\n        messages = messages + [msg]\n      }\n    }\n  }\n\n  onCleanup {\n    socket.get().close()\n  }\n\n  Column {\n    ForEach(messages, key: fn(m, i) { i }) { msg =>\n      Text(msg)\n    }\n  }\n\n",
        );
    }

    #[test]
    fn b3_5_navigation_with_router() {
        check_ok(
            "compo App\n  state route: String = \"home\"\n\n  Router {\n    Screen(\"home\") { Home() }\n    Screen(\"profile\") { Profile() }\n    Screen(\"settings\") { Settings() }\n  }\n\n\ncompo Home\n  let router = useContext(RouterContext)\n\n  Column(gap: 16) {\n    Text(\"Home\")\n    Button(text: \"Open Profile\", onClick: {\n      router.navigate(\"profile\")\n    })\n    Button(text: \"Settings\", onClick: {\n      router.navigate(\"settings\")\n    })\n  }\n\n\ncompo Profile\n  Column { Text(\"Profile\") }\n\n\ncompo Settings\n  Column { Text(\"Settings\") }\n\n",
        );
    }

    #[test]
    fn b3_6_async_with_resource() {
        check_ok(
            "compo UserList\n  let (users, { refetch }) = resource(fn {\n    Api.fetch(\"/users\")\n  })\n\n  Column {\n    when users.is_loading {\n      Text(\"Loading...\")\n    }\n    otherwise {\n      ForEach(users.value, key: fn(u) { u.id }) { user =>\n        Text(\"{user.name}\")\n      }\n    }\n    Button(text: \"Refresh\", onClick: { refetch() })\n  }\n\n",
        );
    }

    #[test]
    fn b3_7_pure_component() {
        check_ok(
            "@pure\ncompo Avatar(url: String, size: Float)\n  Image(url) {\n    width: size,\n    height: size,\n    cornerRadius: size / 2\n  }\n\n\ncompo Profile\n  state avatarUrl: String = \"https://example.com/me.png\"\n\n  Column {\n    Avatar(url: avatarUrl, size: 80)\n    Text(\"Profile\")\n  }\n\n",
        );
    }

    #[test]
    fn b3_8_platform_conditional() {
        check_ok(
            "compo PlatformButton\n  if platform() == \"ios\" {\n    CupertinoButton(text: \"Tap\", onClick: { 1 })\n  } else {\n    MaterialButton(text: \"Tap\", onClick: { 1 })\n  }\n\n",
        );
    }

    #[test]
    fn b3_9_capability_declaration() {
        check_ok(
            "capability Camera {
  fn capture() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  fn set(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn delete(key: String) -> Unit
}
",
        );
    }

    #[test]
    fn b3_10_refs() {
        check_ok(
            "compo LoginForm\n  let emailRef = createRef[TextInput]()\n  let passwordRef = createRef[TextInput]()\n\n  onMount {\n    emailRef.focus()\n  }\n\n  Column(gap: 12) {\n    TextInput(ref: emailRef, placeholder: \"Email\")\n    TextInput(ref: passwordRef, placeholder: \"Password\")\n    Button(text: \"Submit\", onPress: {\n      let email = emailRef.text()\n      let password = passwordRef.text()\n      Auth.login(email, password)\n    })\n  }\n\n",
        );
    }
}

mod diagnostics {
    use super::*;

    #[test]
    fn mismatch_reports_type_error_with_span() {
        let err = check_err("compo Bad\n  let s = 1 + \"not a number\"\n\n");
        // The error must carry a span pointing somewhere in the source.
        assert!(err.span.start > 0 || err.span.end > 0, "span must be set");
        assert!(
            !err.message.is_empty(),
            "diagnostic must explain what went wrong"
        );
    }

    #[test]
    fn non_exhaustive_match_is_rejected() {
        let err = check_err(
            "type Shape =
  | Circle(Float)
  | Square(Float)

fn area(shape: Shape) -> Float {
  match shape {
    Circle(r) => r
  }
}
",
        );
        assert!(
            err.message.contains("non-exhaustive"),
            "expected exhaustiveness error, got: {}",
            err.message
        );
    }

    #[test]
    fn unbound_name_is_rejected() {
        let err = check_err("compo Orphan\n  let x = missingThing() + 1\n\n");
        assert!(
            err.message.contains("unbound"),
            "expected unbound-name error, got: {}",
            err.message
        );
    }

    #[test]
    fn optional_chaining_over_option_is_well_typed() {
        // `user` is `Option[User]`; `user?.name` must type-check. `User` is an
        // opaque single-variant type here, so the checker permissively widens
        // the accessed field to `Option[fresh]` — still well-typed.
        check_ok(
            "type User = User(String)\n\ncompo Profile\n  state user: Option[User] = None\n  Text(user?.name)\n\n",
        );
    }

    #[test]
    fn optional_chaining_requires_option_base() {
        // `n` is `Int` (non-nullable); `n?.foo` is a type error because `?.`
        // only applies to `Option` bases.
        let err = check_err("compo Bad\n  state n: Int = 0\n  Text(n?.foo)\n\n");
        assert!(
            err.message.contains("Option"),
            "expected an Option-base error, got: {}",
            err.message
        );
    }
}

mod structural_records {
    use super::*;

    #[test]
    fn wider_record_assignable_to_narrower_annotation() {
        // FLUX-054: structural (width-subtyping) records. A value carrying
        // extra fields must be assignable where a narrower record type is
        // expected.
        check_ok("compo C\n  state p: { x: Int } = { x: 1, y: 2 }\n  Text(\"ok\")\n\n");
    }

    #[test]
    fn missing_field_still_rejected() {
        // The reverse is still an error: a narrower value lacks a required
        // field of the wider expected type.
        let err = check_err("compo C\n  state p: { x: Int, y: Int } = { x: 1 }\n\n");
        assert!(
            err.message.contains("y") || err.message.to_lowercase().contains("field"),
            "expected a missing-field error, got: {}",
            err.message
        );
    }
}

mod instantiations {
    use super::*;

    #[test]
    fn counter_int_and_float_both_recorded() {
        let source = "trait Numeric[T] {\n  fn zero() -> T\n  fn one() -> T\n}\n\ncompo Counter[T: Numeric](initial: T)\n  state count: T = initial\n\n\ncompo IntCase\n  Counter(initial: 0)\n\n\ncompo FloatCase\n  Counter(initial: 0.0)\n\n";
        let ast = ast_of(source);
        let result = type_check(&ast).expect("fixtures must type-check");
        let names: Vec<&String> = result.instantiations.iter().map(|i| &i.name).collect();
        assert!(
            names.iter().any(|n| n.as_str() == "Counter"),
            "Counter instantiations should be recorded: {names:?}"
        );
        // Both generic-component instantiations must be tracked, with the
        // resolved concrete arguments Int and Float.
        let counter_insts: Vec<&GenericInstantiation> = result
            .instantiations
            .iter()
            .filter(|i| i.name == "Counter")
            .collect();
        assert_eq!(
            counter_insts.len(),
            2,
            "expected two Counter instantiations: {names:?}"
        );
        // Collect the resolved first generic argument of each instantiation.
        let mut args: Vec<String> = counter_insts
            .iter()
            .filter_map(|i| i.generic_args.first())
            .map(|t| t.to_string())
            .collect();
        args.sort();
        assert!(
            args.iter().any(|a| a.contains("Int")),
            "expected an Int instantiation among {args:?}"
        );
        assert!(
            args.iter().any(|a| a.contains("Float")),
            "expected a Float instantiation among {args:?}"
        );
    }

    #[test]
    fn const_color_module_constant() {
        check_ok(
            "type Color = RGB(Float, Float, Float)\nColor.red = RGB(1.0, 0.0, 0.0)\nColor.green = RGB(0.0, 1.0, 0.0)\n\ncompo Swatch\n  let c = Color.red\n  Text(\"red\")\n\n",
        );
    }

    #[test]
    fn trait_bad_string_not_numeric() {
        // A `Show`-constrained generic used in arithmetic must be rejected:
        // `Numeric` is not among `Show`'s bounds, so `+` is inadmissible. The
        // prop keeps `T` constrained (no unifying assignment), so the bound is
        // enforced at the arithmetic site.
        let err = check_err(
            "trait Show[T] {\n  fn show(value: T) -> String\n}\n\ncompo Bad[T: Show](value: T)\n  let y = value + 1\n\n",
        );
        assert!(
            err.message.contains("Numeric") || err.message.contains("arithmetic"),
            "expected a Numeric-bound error, got: {}",
            err.message
        );
    }
}
