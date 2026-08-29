// main.flux — Flux To-Do example (standalone vertical slice).
//
// Demonstrates every MLP primitive plus the Router capability and a second
// capability call, wired through the real reactive VM:
//   - Text, Button, TextInput, Column, Row, Image, Router, Screen
//   - TextInput is CONTROLLED: `onChangeText: |t| { newTask = t }` writes the
//     typed value into the `newTask` signal (the host dispatches the edit as the
//     handler payload, bound to r0 by the compiler), so the "Add task" handler
//     can read it and append to the first empty slot.
//   - Router.navigate (cap 3 / method 1) drives the visible screen via signal 97
//   - Storage.removeItem (cap 2 / method 3) is a real CALL_CAP on Reset
//
// NOTE ON LIST MODELING (read before porting a "real" todo app):
// the MLP lower pass emits a `ForEach` as an empty splice (see
// crates/flux-ir/tests/lower.rs `foreach_emits_empty_splice`) and handler
// `WRITE_SIGNAL` targets are statically-fixed signal names with no list
// set/remove/slice ops, so a dynamically-sized todo list is NOT expressible in
// the current dev pipeline. This example therefore models the list as five
// FIXED slots (each a `String` + `Bool` signal). "Add task" fills the first
// empty slot from `newTask`; "Toggle"/"Remove" flip or clear a slot's signals.

compo TodoApp
    // Five fixed task slots: a label string and a done flag each.
    state t0: String = "Buy groceries"
    state d0: Bool = false
    state t1: String = "Walk the dog"
    state d1: Bool = false
    state t2: String = "Read a chapter"
    state d2: Bool = false
    state t3: String = "Reply to emails"
    state d3: Bool = false
    state t4: String = "Water the plants"
    state d4: Bool = false
    // Count of filled slots; "Add task" appends to the first empty one.
    state added: Int = 5
    // The text currently typed into the input (controlled by TextInput).
    state newTask: String = ""

    Router initialRouteName: "tasks"
        Screen route: "tasks"
            Column gap: 16.0
                Text text: "Flux To-Do"
                Row gap: 8.0
                    TextInput text: newTask, placeholder: "What needs doing?", enabled: true, onChangeText: |t| { newTask = t }
                    Button text: "Add task", onPress: || {
                        when added == 0 { t0 = newTask
                        added = added + 1 }
                        otherwise { when added == 1 { t1 = newTask
                        added = added + 1 }
                        otherwise { when added == 2 { t2 = newTask
                        added = added + 1 }
                        otherwise { when added == 3 { t3 = newTask
                        added = added + 1 }
                        otherwise { when added == 4 { t4 = newTask
                        added = added + 1 } } } } }
                        newTask = ""
                    }
                Row gap: 8.0
                    Button text: "Reset", onPress: || {
                        t0 = ""
                        d0 = false
                        t1 = ""
                        d1 = false
                        t2 = ""
                        d2 = false
                        t3 = ""
                        d3 = false
                        t4 = ""
                        d4 = false
                        added = 0
                        Storage.removeItem(key: "todos")
                    }
                    Button text: "About", onPress: || { Router.navigate("about") }

                // Slot 0
                Row gap: 8.0
                    Button text: "Toggle", onPress: || { when d0 { d0 = false } otherwise { d0 = true } }
                    Text text: "{t0}"
                    Button text: "Remove", onPress: || { t0 = ""
                    d0 = false }
                // Slot 1
                Row gap: 8.0
                    Button text: "Toggle", onPress: || { when d1 { d1 = false } otherwise { d1 = true } }
                    Text text: "{t1}"
                    Button text: "Remove", onPress: || { t1 = ""
                    d1 = false }
                // Slot 2
                Row gap: 8.0
                    Button text: "Toggle", onPress: || { when d2 { d2 = false } otherwise { d2 = true } }
                    Text text: "{t2}"
                    Button text: "Remove", onPress: || { t2 = ""
                    d2 = false }
                // Slot 3
                Row gap: 8.0
                    Button text: "Toggle", onPress: || { when d3 { d3 = false } otherwise { d3 = true } }
                    Text text: "{t3}"
                    Button text: "Remove", onPress: || { t3 = ""
                    d3 = false }
                // Slot 4
                Row gap: 8.0
                    Button text: "Toggle", onPress: || { when d4 { d4 = false } otherwise { d4 = true } }
                    Text text: "{t4}"
                    Button text: "Remove", onPress: || { t4 = ""
                    d4 = false }

        Screen route: "about"
            Column gap: 16.0
                Text text: "About"
                Image source: "assets/flux.png", width: 96.0, height: 96.0
                Text text: "A real Flux app: 5 tasks, a working input, Router + a capability call."
                Button text: "Back", onPress: || { Router.navigate("tasks") }
