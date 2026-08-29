//! The ten Appendix B.3 grammar examples, reproduced verbatim from
//! `/docs/spec/mlp-appendices.md`, as the parity harness's fixtures.
//!
//! Each example is a standalone `.flux` source exercising one language feature.
//! The harness compiles each through the full pipeline and checks that the dev
//! (lowered-IR) tree and both release (Swift/Kotlin codegen) trees are
//! structurally equivalent.

/// B.3.1 — simple component declaring `state` and a `Column` tree, with a string
/// interpolation and an `onPress` handler.
pub(crate) const B31_SIMPLE: &str = r#"compo HelloWorld
  state count: Int = 0

  Column(gap: 12) {
    Text("Count: {count}")
    Button(text: "Increment", onPress: {
      count = count + 1
    })
}"#;

/// B.3.2 — generic component with a trait bound, emitting `Column`/`Text`/`Button`.
pub(crate) const B32_GENERIC: &str = r#"trait Numeric[T] {
  fn zero() -> T
  fn one() -> T
  fn +(a: T, b: T) -> T
  fn -(a: T, b: T) -> T
}

compo Counter[T: Numeric]
  state count: T = Numeric.zero()

  Column(gap: 8) {
    Text("Count: {count}")
    Button(text: "+", onPress: { count = count + Numeric.one() })
    Button(text: "-", onPress: { count = count - Numeric.one() })
}"#;

/// B.3.3 — ADT and pattern matching over every arm.
pub(crate) const B33_ADT: &str = r#"type Shape =
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

compo ShapeDisplay
  state shape: Shape = Circle(5.0)

  Column {
    Text("Area: {area(shape)}")
    Button(text: "Make Square", onPress: {
      shape = Rectangle(4.0, 4.0)
    })
}"#;

/// B.3.4 — lifecycle (`onMount`/`onCleanup`), `createRef`, and a `ForEach`.
pub(crate) const B34_LIFECYCLE: &str = r#"compo Chat
  state messages: List[String] = []
  let socket = createRef[WebSocket]()

  onMount {
    socket.set(WebSocket.connect("ws://localhost:8080"))
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
"#;

/// B.3.5 — navigation with a `Router`, `Screen`s, and `useContext`. The spec
/// example calls `Home()`/`Profile()`/`Settings()` inside the screens; those are
/// declared as minimal components here so the source is self-contained and
/// type-checks (the parity harness needs a compiling source, not just a parse).
pub(crate) const B35_NAVIGATION: &str = r#"compo App
  state route: String = "home"

  Router {
    Screen("home") { Home() }
    Screen("profile") { Profile() }
    Screen("settings") { Settings() }
}

compo Home
  let router = useContext(RouterContext)

  Column(gap: 16) {
    Text("Home")
    Button(text: "Open Profile", onPress: {
      router.navigate("profile")
    })
}

compo Profile
  Column { Text("Profile") }

compo Settings
  Column { Text("Settings") }"#;

/// B.3.6 — async with `resource`, `when … otherwise`, and a `ForEach`.
pub(crate) const B36_ASYNC: &str = r#"compo UserList
  let (users, { refetch }) = resource(fn {
    Api.fetch("/users")
  })

  Column {
    when users.is_loading {
      Text("Loading...")
    }
    otherwise {
      ForEach(users.value, key: fn(u) { u.id }) { user =>
        Text("{user.name}")
      }
    }
    Button(text: "Refresh", onPress: { refetch() })
  }
"#;

/// B.3.7 — `@pure` component with a prop block (`Image(url) { width: size }`).
pub(crate) const B37_PURE: &str = r#"@pure
compo Avatar(url: String, size: Float)
  Image(source: url) {
    width: size,
    height: size,
    cornerRadius: size / 2
}

compo Profile
  state avatarUrl: String = "https://example.com/me.png"

  Column {
    Avatar(url: avatarUrl, size: 80)
    Text("Profile")
}"#;

/// B.3.8 — platform conditional routing between two native components.
pub(crate) const B38_PLATFORM: &str = r#"compo PlatformButton
  if platform() == "ios" {
    CupertinoButton(text: "Tap", onPress: { ... })
  } else {
    MaterialButton(text: "Tap", onPress: { ... })
}"#;

/// B.3.9 — capability declarations listing every method.
pub(crate) const B39_CAPABILITY: &str = r#"capability Camera {
  fn capture() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  fn set(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn delete(key: String) -> Unit
}"#;

/// B.3.10 — refs via `createRef[TextInput]()` and binding them in `onMount`.
pub(crate) const B310_REFS: &str = r#"compo LoginForm
  let emailRef = createRef[TextInput]()
  let passwordRef = createRef[TextInput]()

  onMount {
    emailRef.focus()
  }

  Column(gap: 12) {
    TextInput(ref: emailRef, placeholder: "Email")
    TextInput(ref: passwordRef, placeholder: "Password")
    Button(text: "Submit", onPress: {
      let email = emailRef.text()
      let password = passwordRef.text()
      Auth.login(email, password)
    })
  }
"#;

/// All ten examples, in B.3.1 → B.3.10 order, as `(name, source)` pairs.
#[must_use]
pub fn all_examples() -> &'static [(&'static str, &'static str)] {
    &[
        ("b31_simple", B31_SIMPLE),
        ("b32_generic", B32_GENERIC),
        ("b33_adt", B33_ADT),
        ("b34_lifecycle", B34_LIFECYCLE),
        ("b35_navigation", B35_NAVIGATION),
        ("b36_async", B36_ASYNC),
        ("b37_pure", B37_PURE),
        ("b38_platform", B38_PLATFORM),
        ("b39_capability", B39_CAPABILITY),
        ("b310_refs", B310_REFS),
    ]
}
