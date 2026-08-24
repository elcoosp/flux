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
            "component HelloWorld {
  state count: Int = 0

  Column(gap: 12) {
    Text(\"Count: {count}\")
    Button(text: \"Increment\", onClick: {
      count = count + 1
    })
  }
}
",
        );
    }

    #[test]
    fn b3_2_generic_component_trait_bound() {
        check_ok(
            "trait Numeric[T] {
  fn zero() -> T
  fn one() -> T
  fn +(a: T, b: T) -> T
  fn -(a: T, b: T) -> T
}

component Counter[T: Numeric] {
  state count: T = Numeric.zero()

  Column(gap: 8) {
    Text(\"Count: {count}\")
    Button(text: \"+\", onClick: { count = count + Numeric.one() })
    Button(text: \"−\", onClick: { count = count - Numeric.one() })
  }
}
",
        );
    }

    #[test]
    fn b3_3_algebraic_data_type_and_match() {
        check_ok(
            "type Shape =
  | Circle(Float)
  | Rectangle(Float, Float)
  | Triangle(Float, Float, Float)

fn area(shape: Shape) -> Float {
  match shape {
    Circle(r) => 3.14159 * r * r
    Rectangle(w, h) => w * h
    Triangle(b, h, _) => 0.5 * b * h
  }
}

component ShapeDisplay {
  state shape: Shape = Circle(5.0)

  Column {
    Text(\"Area: {area(shape)}\")
    Button(text: \"Make Square\", onClick: {
      shape = Rectangle(4.0, 4.0)
    })
  }
}
",
        );
    }

    #[test]
    fn b3_4_lifecycle_effects_cleanup() {
        check_ok(
            "component Chat {
  state messages: List[String] = []
  let socket = createRef[WebSocket]()

  onMount {
    socket.set(WebSocket.connect(\"ws://localhost:8080\"))
    socket.get().on_message = fn(msg: String) {
      batch {
        messages = messages + [msg]
      }
    }
  }

  onCleanup {
    socket.get().close()
  }

  Column {
    ForEach(messages, key: fn(m, i) { i }) { msg =>
      Text(msg)
    }
  }
}
",
        );
    }

    #[test]
    fn b3_5_navigation_with_router() {
        check_ok(
            "component App {
  state route: String = \"home\"

  Router {
    Screen(\"home\") { Home() }
    Screen(\"profile\") { Profile() }
    Screen(\"settings\") { Settings() }
  }
}

component Home {
  let router = useContext(RouterContext)

  Column(gap: 16) {
    Text(\"Home\")
    Button(text: \"Open Profile\", onClick: {
      router.navigate(\"profile\")
    })
    Button(text: \"Settings\", onClick: {
      router.navigate(\"settings\")
    })
  }
}

component Profile {
  Column { Text(\"Profile\") }
}

component Settings {
  Column { Text(\"Settings\") }
}
",
        );
    }

    #[test]
    fn b3_6_async_with_resource() {
        check_ok(
            "component UserList {
  let (users, { refetch }) = resource(fn {
    Api.fetch(\"/users\")
  })

  Column {
    when users.is_loading {
      Text(\"Loading...\")
    }
    otherwise {
      ForEach(users.value, key: fn(u) { u.id }) { user =>
        Text(\"{user.name}\")
      }
    }
    Button(text: \"Refresh\", onClick: { refetch() })
  }
}
",
        );
    }

    #[test]
    fn b3_7_pure_component() {
        check_ok(
            "@pure
component Avatar(url: String, size: Float) {
  Image(url) {
    width: size,
    height: size,
    cornerRadius: size / 2
  }
}

component Profile {
  state avatarUrl: String = \"https://example.com/me.png\"

  Column {
    Avatar(url: avatarUrl, size: 80)
    Text(\"Profile\")
  }
}
",
        );
    }

    #[test]
    fn b3_8_platform_conditional() {
        check_ok(
            "component PlatformButton {
  if platform() == \"ios\" {
    CupertinoButton(text: \"Tap\", onClick: { 1 })
  } else {
    MaterialButton(text: \"Tap\", onClick: { 1 })
  }
}
",
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
            "component LoginForm {
  let emailRef = createRef[TextField]()
  let passwordRef = createRef[TextField]()

  onMount {
    emailRef.focus()
  }

  Column(gap: 12) {
    TextField(ref: emailRef, placeholder: \"Email\")
    TextField(ref: passwordRef, placeholder: \"Password\")
    Button(text: \"Submit\", onClick: {
      let email = emailRef.text()
      let password = passwordRef.text()
      Auth.login(email, password)
    })
  }
}
",
        );
    }
}

mod diagnostics {
    use super::*;

    #[test]
    fn mismatch_reports_type_error_with_span() {
        let err = check_err(
            "component Bad {
  let s = 1 + \"not a number\"
}
",
        );
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
        let err = check_err(
            "component Orphan {
  let x = missingThing() + 1
}
",
        );
        assert!(
            err.message.contains("unbound"),
            "expected unbound-name error, got: {}",
            err.message
        );
    }
}

mod instantiations {
    use super::*;

    #[test]
    fn counter_int_and_float_both_recorded() {
        let source = "trait Numeric[T] {
  fn zero() -> T
  fn one() -> T
}

component Counter[T: Numeric](initial: T) {
  state count: T = initial
}

component IntCase {
  Counter(initial: 0)
}

component FloatCase {
  Counter(initial: 0.0)
}
";
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
            "type Color = RGB(Float, Float, Float)
Color.red = RGB(1.0, 0.0, 0.0)
Color.green = RGB(0.0, 1.0, 0.0)

component Swatch {
  let c = Color.red
  Text(\"red\")
}
",
        );
    }

    #[test]
    fn trait_bad_string_not_numeric() {
        // A `Show`-constrained generic used in arithmetic must be rejected:
        // `Numeric` is not among `Show`'s bounds, so `+` is inadmissible. The
        // prop keeps `T` constrained (no unifying assignment), so the bound is
        // enforced at the arithmetic site.
        let err = check_err(
            "trait Show[T] {
  fn show(value: T) -> String
}

component Bad[T: Show](value: T) {
  let y = value + 1
}
",
        );
        assert!(
            err.message.contains("Numeric") || err.message.contains("arithmetic"),
            "expected a Numeric-bound error, got: {}",
            err.message
        );
    }
}
