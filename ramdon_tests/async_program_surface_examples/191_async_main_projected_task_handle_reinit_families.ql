struct Slot {
    value: Int,
}

struct Pending {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var literal_tasks = [worker(1), worker(2)]
    let literal_first = await literal_tasks[0]
    literal_tasks[0] = worker(literal_first + 2)
    let literal_second = await literal_tasks[0]

    let flag = true
    var conditional_literal_tasks = [worker(3), worker(4)]
    if flag {
        let conditional_literal_first = await conditional_literal_tasks[0]
        conditional_literal_tasks[0] = worker(conditional_literal_first + 2)
    }
    let conditional_literal_final = await conditional_literal_tasks[0]

    let slot = Slot { value: 0 }

    var stable_dynamic_tasks = [worker(5), worker(6)]
    let stable_dynamic_first = await stable_dynamic_tasks[slot.value]
    stable_dynamic_tasks[slot.value] = worker(stable_dynamic_first + 2)
    let stable_dynamic_second = await stable_dynamic_tasks[slot.value]

    var conditional_dynamic_tasks = [worker(7), worker(8)]
    if flag {
        let conditional_dynamic_first = await conditional_dynamic_tasks[slot.value]
        conditional_dynamic_tasks[slot.value] = worker(conditional_dynamic_first + 2)
    }
    let conditional_dynamic_final = await conditional_dynamic_tasks[slot.value]

    var pending = Pending {
        tasks: [worker(9), worker(10)],
    }
    let projected_root_first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker(projected_root_first + 2)
    let projected_root_second = await pending.tasks[slot.value]

    return literal_first
        + literal_second
        + conditional_literal_final
        + stable_dynamic_first
        + stable_dynamic_second
        + conditional_dynamic_final
        + projected_root_first
        + projected_root_second
}
