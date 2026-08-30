// prelude.flux — default-imported basic types for every Flux module.
//
// Per mlp-spec §18.3, `flux::prelude` is imported implicitly into every
// module. The stdlib as a whole behaves as that prelude: the primitive
// scalar types (Int, Float, Bool, String, Unit), the collection types
// (List, Map, Option), the stdlib traits (Numeric, Eq, Show — see
// traits.flux), the adapter components (Text, Button, Column, Row,
// TextInput, Router, Screen, Switch, Checkbox, Slider, Picker, DatePicker,
// TextArea, Gesture — see the corresponding files; Switch/Checkbox/Slider/
// Picker/DatePicker/TextArea (FLUX-040) and Gesture (FLUX-041) gained their
// iOS adapter parity in FLUX-076, clearing the both-kits advertising gate),
// and the auxiliary value types declared in this file are all in scope without
// an explicit import.
//
// This file declares the auxiliary value types referenced by the adapter
// prop contracts in Appendix F. They are algebraic data types so that the
// codegen pass can map each variant onto the platform spelling
// (e.g. Alignment.Center -> SwiftUI .center, Compose Alignment.Center).
//
// List, Map, and Option are language-intrinsic collection types (mlp-spec
// §18.3 "Stdlib types (imported by default)"); Option's constructors are
// declared here so that `None` / `Some(..)` are usable as values, and so
// that the `= None` prop defaults in Appendix F resolve.

// Optional value — `None` or `Some(value)`.
type Option[T] = | None | Some(T)

// Text alignment along one axis; maps to platform alignment enums.
type Alignment =
  | Leading
  | Center
  | Trailing
  | Top
  | Bottom
  | Fill

// Text overflow behavior when content exceeds `maxLines`.
type Overflow =
  | Clip
  | Ellipsis
  | Visible

// Software keyboard flavor requested by a `TextInput`.
type KeyboardType =
  | Default
  | Email
  | Numeric
  | Phone
  | Url

// How an `Image` fits its bounding box (Appendix F.8, deferred to MLP v2).
type ContentMode =
  | Fit
  | Fill
  | Center

// Opaque platform reference to a native view, produced by `createRef`.
// `T` is the referenced view kind (e.g. `Ref[TextInput]`).
type Ref[T] = Ref(T)

// An interactive callback bound to a host handler id. The wrapped function
// is the handler body executed when the event fires.
type Handler = Handler(Fn() -> Unit)

// Raw binary payload exchanged with capabilities (Appendix §24).
type Data = Data(List[Int])
