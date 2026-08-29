---
title: TextInput
description: A controlled text field that fires onChangeText on every edit.
---

Contract (from `stdlib/text_field.flux`; the public name mirrors React Native's
`TextInput` — see [ADR-0038](https://github.com/elcoosp/flux/tree/main/docs/adr)):

| Prop | Type | Default | Notes |
|---|---|---|---|
| `text` | `String` | `""` | controlled value |
| `onChangeText` | `Handler` | — | required; fired with the new text |
| `placeholder` | `Option[String]` | `None` | hint text |
| `ref` | `Option[Ref[TextInput]]` | `None` | native view handle |
| `enabled` | `Bool` | `true` | editable when `true` |
| `secureTextEntry` | `Bool` | `false` | password masking |
| `keyboardType` | `Option[KeyboardType]` | `None` | soft-keyboard flavor |

`TextInput` is **controlled**: the host dispatches each keystroke as the
`onChangeText` handler's argument, and you write it into a signal. The rendered
value is whatever you bound to `text` — so to make the field reflect typing,
write the handler payload back into the signal you pass as `text`.

```flux
compo AddTask
  state newTask: String = ""
  Row
    TextInput text: newTask, placeholder: "What needs doing?",
      onChangeText: |t| { newTask = t }
    Button text: "Add", onPress: || { /* use newTask */ }
```

The `onChangeText` closure receives the new text as its single argument (bound to
`r0` by the compiler). This exact pattern drives the input in the
[Todo example](https://github.com/elcoosp/flux/tree/main/examples/todo). Native
rendering: `UITextField` / `EditText` in dev; SwiftUI `TextField` / Compose
`TextField` in release (Appendix F.5).
