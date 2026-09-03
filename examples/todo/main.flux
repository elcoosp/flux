// main.flux — minimal verified todo (no Box/RGB/Toggle) to restore iOS/Android render
record Task { label: String, done: Bool, }

compo TaskRow(task: Task, tasks: List[Task])
    Row gap: 8.0
        Text text: task.label
        Button text: "Remove", onPress: || { tasks.remove(task) }

compo TodoApp
    state tasks: List[Task] = [
        Task(label: "Buy milk", done: false),
        Task(label: "Walk dog", done: false),
        Task(label: "Do taxes", done: false),
        Task(label: "Call mom", done: false),
    ]
    state newTask: String = ""
    Router initialRouteName: "tasks"
        Screen route: "tasks"
            Column gap: 12.0
                Text text: "Flux To-Do"
                Row gap: 8.0
                    TextInput text: $newTask, onChangeText: fn(v) { newTask = v }, placeholder: "What needs doing?"
                    Button text: "Add", onPress: || {
                        tasks.append(Task(label: newTask, done: false))
                        newTask = ""
                    }
                Column gap: 8.0
                    ForEach(tasks, key: fn(t) { t.label }) { item => TaskRow(task: item, tasks: tasks) }
                Button text: "Reset", onPress: || {
                    tasks.clear()
                    Storage.removeItem(key: "todos")
                }
                Button text: "About", onPress: || { Router.navigate("about") }
        Screen route: "about"
            Column gap: 12.0
                Text text: "About"
                Text text: "A real Flux app"
                Button text: "Back", onPress: || { Router.navigate("tasks") }

