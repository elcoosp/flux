//! Appendix B.3.4–B.3.10 examples: reactivity, navigation, async, platform.
//!
//! These are the FLUX-003 acceptance tests: each example is reproduced
//! verbatim from `/docs/spec/mlp-appendices.md` §B.3 and asserted against the
//! shape the parser produces.

use flux_parser::{BlockItem, Expr, ExprKind, LifecycleKind};

/// Appendix B.3 source for `b34_lifecycle_effects_and_cleanup_parse_as_lifecycle_expressions`.
mod common;

use common::{component, parse_ok};

const B34_SOURCE: &str = r#"component Chat {
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
}"#;

/// Appendix B.3 source for `b35_navigation_with_router_parses_nested_screens_and_use_context`.
const B35_SOURCE: &str = r#"component App {
  state route: String = "home"

  Router {
    Screen("home") { Home() }
    Screen("profile") { Profile() }
    Screen("settings") { Settings() }
  }
}

component Home {
  let router = useContext(RouterContext)

  Column(gap: 16) {
    Text("Home")
    Button(text: "Open Profile", onClick: {
      router.navigate("profile")
    })
  }
}"#;

#[test]
fn b34_lifecycle_effects_and_cleanup_parse_as_lifecycle_expressions() {
    let ast = parse_ok(B34_SOURCE);

    let decl = component(&ast, 0);
    let kinds: Vec<LifecycleKind> = decl
        .body
        .items
        .iter()
        .filter_map(|item| match item {
            BlockItem::Expr(Expr {
                kind: ExprKind::Lifecycle { kind, .. },
                ..
            }) => Some(*kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        vec![LifecycleKind::OnMount, LifecycleKind::OnCleanup]
    );
}

#[test]
fn b34_for_each_captures_its_key_function_and_loop_binding() {
    let ast = parse_ok(
        r#"component Chat {
  Column {
    ForEach(messages, key: fn(m, i) { i }) { msg =>
      Text(msg)
    }
  }
}"#,
    );
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { trailing, .. },
        ..
    }) = &component(&ast, 0).body.items[0]
    else {
        panic!("expected a `Column` call with a trailing block");
    };
    let column = trailing.as_ref().expect("Column has a trailing block");
    let BlockItem::Expr(Expr {
        kind: ExprKind::ForEach { key, body, .. },
        ..
    }) = &column.items[0]
    else {
        panic!("expected a ForEach expression");
    };
    assert!(matches!(key.kind, ExprKind::Lambda { .. }));
    assert_eq!(body.params.len(), 1);
}

#[test]
fn b35_navigation_with_router_parses_nested_screens_and_use_context() {
    let ast = parse_ok(B35_SOURCE);

    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { trailing, .. },
        ..
    }) = &component(&ast, 0).body.items[1]
    else {
        panic!("expected a `Router` call with a trailing block");
    };
    assert_eq!(trailing.as_ref().expect("Router body").items.len(), 3);

    let BlockItem::Expr(Expr {
        kind: ExprKind::Let {
            value: Some(value), ..
        },
        ..
    }) = &component(&ast, 1).body.items[0]
    else {
        panic!("expected a `let` binding");
    };
    assert!(matches!(value.kind, ExprKind::UseContext(_)));
}

#[test]
fn b36_async_with_resource_destructures_the_resource_pair() {
    let ast = parse_ok(
        r#"component UserList {
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
    Button(text: "Refresh", onClick: { refetch() })
  }
}"#,
    );

    let decl = component(&ast, 0);
    let BlockItem::Expr(Expr {
        kind: ExprKind::Let {
            pattern,
            value: Some(value),
        },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected the resource `let`");
    };
    assert!(matches!(pattern, flux_parser::LetPattern::Tuple(items) if items.len() == 2));
    assert!(matches!(value.kind, ExprKind::Resource(_)));
}

#[test]
fn b36_when_otherwise_binds_both_branches() {
    let ast = parse_ok(
        r#"component A {
  when loading {
    Text("Loading...")
  }
  otherwise {
    Text("Done")
  }
}"#,
    );
    let BlockItem::Expr(Expr {
        kind: ExprKind::When { otherwise, .. },
        ..
    }) = &component(&ast, 0).body.items[0]
    else {
        panic!("expected a `when` expression");
    };
    assert!(otherwise.is_some());
}

#[test]
fn b37_pure_component_records_the_annotation_and_prop_block() {
    let ast = parse_ok(
        r#"@pure
component Avatar(url: String, size: Float) {
  Image(url) {
    width: size,
    height: size,
    cornerRadius: size / 2
  }
}

component Profile {
  state avatarUrl: String = "https://example.com/me.png"

  Column {
    Avatar(url: avatarUrl, size: 80)
    Text("Profile")
  }
}"#,
    );

    let avatar = component(&ast, 0);
    assert_eq!(avatar.annotations[0].name.name, "pure");
    assert_eq!(avatar.props.len(), 2);
    assert_eq!(avatar.props[1].name.name, "size");

    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { trailing, .. },
        ..
    }) = &avatar.body.items[0]
    else {
        panic!("expected an `Image` call with a prop block");
    };
    let props = trailing.as_ref().expect("Image prop block");
    assert!(matches!(&props.items[0], BlockItem::Prop { name, .. } if name.name == "width"));
    assert_eq!(props.items.len(), 3);
}
