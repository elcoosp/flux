compo Counter
    $count: Int = 0

    Column gap: 8.0
        Text text: "tapped {count} times"
        Button text: "Increment", onClick: || { count = count + 1 }
