struct Slot {
    value: Int,
}

struct Pending {
    tasks: [Task[Int]; 2],
}

const INDEX: Int = 0
const STEP: Int = 1
const ARITH_INDEX: Int = STEP - 1

async fn worker(value: Int) -> Int {
    return value
}

async fn direct_guard_refine(index: Int) -> Int {
    var tasks = [worker(1), worker(2)]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    let tail = await tasks[1]
    return final_value + tail
}

async fn projected_guard_refine() -> Int {
    var tasks = [worker(3), worker(4)]
    let slot = Slot { value: 0 }
    if slot.value == 0 {
        let first = await tasks[slot.value]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    let tail = await tasks[1]
    return final_value + tail
}

async fn aliased_projected_root_guard_refine() -> Int {
    var pending = Pending {
        tasks: [worker(5), worker(6)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 1)
    }
    let final_value = await alias[0]
    let tail = await pending.tasks[1]
    return final_value + tail
}

async fn const_backed_alias_root_guard_refine() -> Int {
    var pending = Pending {
        tasks: [worker(7), worker(8)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 1)
    }
    let final_value = await alias[0]
    let tail = await pending.tasks[1]
    return final_value + tail
}

async fn arithmetic_const_guard_refine() -> Int {
    var tasks = [worker(9), worker(10)]
    if ARITH_INDEX == 0 {
        let first = await tasks[ARITH_INDEX]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    let tail = await tasks[1]
    return final_value + tail
}

async fn arithmetic_projected_guard_refine() -> Int {
    var tasks = [worker(11), worker(12)]
    let slot = Slot { value: 2 - 2 }
    if slot.value == 0 {
        let first = await tasks[slot.value]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    let tail = await tasks[1]
    return final_value + tail
}

async fn main() -> Int {
    let first = await direct_guard_refine(0)
    let second = await projected_guard_refine()
    let third = await aliased_projected_root_guard_refine()
    let fourth = await const_backed_alias_root_guard_refine()
    let fifth = await arithmetic_const_guard_refine()
    let sixth = await arithmetic_projected_guard_refine()
    return first + second + third + fourth + fifth + sixth
}
