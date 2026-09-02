// main.flux — Flux To-Do example (FLUX-072 concise data-driven surface).

record Task {
    label: String,
    done: Bool,
}

compo TaskRow(task: Task, tasks: List[Task])
    Row gap: 8.0
        Toggle value: task.done, onValueChange: fn(v) { task.done = !task.done }
        Text text: task.label
        Spacer weight: 1.0
        Button text: "Remove", onPress: || { tasks.remove(task) }

compo TodoApp
    state tasks: List[Task] = [
        Task(label: "Buy milk", done: false),
        Task(label: "Walk dog", done: false),
        Task(label: "Do taxes", done: false),
        Task(label: "Call mom", done: false),
    ]
    state newTask: String = ""
    derived hasTasks = !tasks.isEmpty

    Router initialRouteName: "tasks"
        Screen route: "tasks"
            Column gap: 16.0
                Text text: "Flux To-Do"
                Row gap: 8.0
                    TextInput text: $newTask, onChangeText: fn(v) { newTask = v }, placeholder: "What needs doing?"
                    Button text: "Add task", onPress: || {
                        tasks.append(Task(label: newTask, done: false))
                        newTask = ""
                    }
                ForEach(tasks, key: fn(t) { t.label }) { item =>
                    TaskRow(task: item, tasks: tasks)
                }
                Button text: "Reset", onPress: || {
                    tasks.clear()
                    Storage.removeItem(key: "todos")
                }
                Button text: "About", onPress: || { Router.navigate("about") }

        Screen route: "about"
            Column gap: 16.0
                Text text: "About"
                Text text: "A real Flux app: a dynamic todo list, two-way input, Router + a capability call."
                Button text: "Back", onPress: || { Router.navigate("tasks") }
