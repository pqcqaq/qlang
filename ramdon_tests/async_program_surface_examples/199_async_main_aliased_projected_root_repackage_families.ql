use ARITH_INDEX as ARITH_INDEX_ALIAS
use ARITH_SLOT as ARITH_SLOT_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

struct ArrayBundle {
    tasks: [Task[Int]; 2],
}

struct ArrayEnvelope {
    bundle: ArrayBundle,
    tail: Task[Int],
}

const STEP: Int = 1
const ARITH_INDEX: Int = STEP - 1
static BASE: Int = 2
static ARITH_SLOT: Slot = Slot { value: BASE - 2 }

async fn worker(value: Int) -> Int {
    return value
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let slot = Slot { value: 0 }

    var tuple_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let tuple_alias = tuple_pending.tasks
    let tuple_first = await tuple_alias[slot.value]
    tuple_pending.tasks[slot.value] = worker(tuple_first + 3)
    let tuple_pair = (tuple_alias[slot.value], worker(5))
    let tuple_second = await tuple_pair[0]
    let tuple_extra = await tuple_pair[1]
    let tuple_tail = await tuple_pending.tasks[1]

    var struct_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let struct_alias = struct_pending.tasks
    let struct_first = await struct_alias[slot.value]
    struct_pending.tasks[slot.value] = worker(struct_first + 3)
    let struct_bundle = Bundle {
        left: struct_alias[slot.value],
        right: worker(6),
    }
    let struct_second = await struct_bundle.left
    let struct_extra = await struct_bundle.right
    let struct_tail = await struct_pending.tasks[1]

    var nested_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let nested_alias = nested_pending.tasks
    let nested_first = await nested_alias[slot.value]
    nested_pending.tasks[slot.value] = worker(nested_first + 3)
    let nested_env = Envelope {
        bundle: Bundle {
            left: nested_alias[slot.value],
            right: worker(7),
        },
        tail: nested_pending.tasks[1],
    }
    let nested_second = await nested_env.bundle.left
    let nested_extra = await nested_env.bundle.right
    let nested_tail = await nested_env.tail

    var arithmetic_const_pending = Pending {
        tasks: [worker(3), worker(4)],
    }
    let arithmetic_const_alias = arithmetic_const_pending.tasks
    let arithmetic_const_first = await arithmetic_const_alias[ARITH_INDEX_ALIAS]
    arithmetic_const_pending.tasks[0] = worker(arithmetic_const_first + 2)
    let arithmetic_const_pair = (arithmetic_const_alias[ARITH_INDEX_ALIAS], worker(6))
    let arithmetic_const_second = await arithmetic_const_pair[0]
    let arithmetic_const_extra = await arithmetic_const_pair[1]
    let arithmetic_const_tail = await arithmetic_const_pending.tasks[1]

    var arithmetic_static_pending = Pending {
        tasks: [worker(2), worker(5)],
    }
    let arithmetic_static_alias = arithmetic_static_pending.tasks
    let arithmetic_static_first = await arithmetic_static_alias[ARITH_SLOT_ALIAS.value]
    arithmetic_static_pending.tasks[0] = worker(arithmetic_static_first + 3)
    let arithmetic_static_env = Envelope {
        bundle: Bundle {
            left: arithmetic_static_alias[ARITH_SLOT_ALIAS.value],
            right: worker(7),
        },
        tail: arithmetic_static_pending.tasks[1],
    }
    let arithmetic_static_second = await arithmetic_static_env.bundle.left
    let arithmetic_static_extra = await arithmetic_static_env.bundle.right
    let arithmetic_static_tail = await arithmetic_static_env.tail

    let arithmetic_const_row = ARITH_INDEX_ALIAS
    let arithmetic_const_slots = [arithmetic_const_row, arithmetic_const_row]
    let arithmetic_const_slot_alias = arithmetic_const_slots
    var arithmetic_const_composed_pending = Pending {
        tasks: [worker(1), worker(4)],
    }
    let arithmetic_const_composed_alias = arithmetic_const_composed_pending.tasks
    let arithmetic_const_composed_first =
        await arithmetic_const_composed_alias[arithmetic_const_slot_alias[arithmetic_const_row]]
    arithmetic_const_composed_pending.tasks[arithmetic_const_slots[arithmetic_const_row]] =
        worker(arithmetic_const_composed_first + 2)
    let arithmetic_const_composed_pair = (
        arithmetic_const_composed_alias[arithmetic_const_slot_alias[arithmetic_const_row]],
        worker(5),
    )
    let arithmetic_const_composed_second = await arithmetic_const_composed_pair[0]
    let arithmetic_const_composed_extra = await arithmetic_const_composed_pair[1]
    let arithmetic_const_composed_tail = await arithmetic_const_composed_pending.tasks[1]

    let arithmetic_static_row = ARITH_SLOT_ALIAS.value
    let arithmetic_static_slots = [arithmetic_static_row, arithmetic_static_row]
    let arithmetic_static_slot_alias = arithmetic_static_slots
    var arithmetic_static_composed_pending = Pending {
        tasks: [worker(2), worker(6)],
    }
    let arithmetic_static_composed_alias = arithmetic_static_composed_pending.tasks
    let arithmetic_static_composed_first =
        await arithmetic_static_composed_alias[arithmetic_static_slot_alias[arithmetic_static_row]]
    arithmetic_static_composed_pending.tasks[arithmetic_static_slots[arithmetic_static_row]] =
        worker(arithmetic_static_composed_first + 3)
    let arithmetic_static_composed_env = Envelope {
        bundle: Bundle {
            left: arithmetic_static_composed_alias[arithmetic_static_slot_alias[arithmetic_static_row]],
            right: worker(7),
        },
        tail: arithmetic_static_composed_pending.tasks[1],
    }
    let arithmetic_static_composed_second = await arithmetic_static_composed_env.bundle.left
    let arithmetic_static_composed_extra = await arithmetic_static_composed_env.bundle.right
    let arithmetic_static_composed_tail = await arithmetic_static_composed_env.tail

    let guarded_arithmetic_const_row = ARITH_INDEX_ALIAS
    let guarded_arithmetic_const_slots = [guarded_arithmetic_const_row, guarded_arithmetic_const_row]
    let guarded_arithmetic_const_slot_alias = guarded_arithmetic_const_slots
    var guarded_arithmetic_const_pending = Pending {
        tasks: [worker(1), worker(3)],
    }
    let guarded_arithmetic_const_alias = guarded_arithmetic_const_pending.tasks
    if ARITH_INDEX_ALIAS == 0 {
        let guarded_arithmetic_const_first =
            await guarded_arithmetic_const_alias[guarded_arithmetic_const_slot_alias[guarded_arithmetic_const_row]]
        guarded_arithmetic_const_pending.tasks[guarded_arithmetic_const_slots[guarded_arithmetic_const_row]] =
            worker(guarded_arithmetic_const_first + 1)
    }
    let guarded_arithmetic_const_pair = (
        guarded_arithmetic_const_alias[guarded_arithmetic_const_slot_alias[guarded_arithmetic_const_row]],
        worker(4),
    )
    let guarded_arithmetic_const_second = await guarded_arithmetic_const_pair[0]
    let guarded_arithmetic_const_extra = await guarded_arithmetic_const_pair[1]
    let guarded_arithmetic_const_tail = await guarded_arithmetic_const_pending.tasks[1]

    let guarded_arithmetic_static_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_static_slots = [guarded_arithmetic_static_row, guarded_arithmetic_static_row]
    let guarded_arithmetic_static_slot_alias = guarded_arithmetic_static_slots
    var guarded_arithmetic_static_pending = Pending {
        tasks: [worker(2), worker(4)],
    }
    let guarded_arithmetic_static_alias = guarded_arithmetic_static_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_static_first =
            await guarded_arithmetic_static_alias[guarded_arithmetic_static_slot_alias[guarded_arithmetic_static_row]]
        guarded_arithmetic_static_pending.tasks[guarded_arithmetic_static_slots[guarded_arithmetic_static_row]] =
            worker(guarded_arithmetic_static_first + 2)
    }
    let guarded_arithmetic_static_env = Envelope {
        bundle: Bundle {
            left: guarded_arithmetic_static_alias[guarded_arithmetic_static_slot_alias[guarded_arithmetic_static_row]],
            right: worker(5),
        },
        tail: guarded_arithmetic_static_pending.tasks[1],
    }
    let guarded_arithmetic_static_second = await guarded_arithmetic_static_env.bundle.left
    let guarded_arithmetic_static_extra = await guarded_arithmetic_static_env.bundle.right
    let guarded_arithmetic_static_tail = await guarded_arithmetic_static_env.tail

    let guarded_arithmetic_bundle_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_bundle_slots = [guarded_arithmetic_bundle_row, guarded_arithmetic_bundle_row]
    let guarded_arithmetic_bundle_slot_alias = guarded_arithmetic_bundle_slots
    var guarded_arithmetic_bundle_pending = Pending {
        tasks: [worker(2), worker(5)],
    }
    let guarded_arithmetic_bundle_alias = guarded_arithmetic_bundle_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_bundle_first =
            await guarded_arithmetic_bundle_alias[guarded_arithmetic_bundle_slot_alias[guarded_arithmetic_bundle_row]]
        guarded_arithmetic_bundle_pending.tasks[guarded_arithmetic_bundle_slots[guarded_arithmetic_bundle_row]] =
            worker(guarded_arithmetic_bundle_first + 4)
    }
    let guarded_arithmetic_bundle_tail_tasks = guarded_arithmetic_bundle_pending.tasks
    let guarded_arithmetic_bundle_forwarded =
        forward(guarded_arithmetic_bundle_alias[guarded_arithmetic_bundle_slot_alias[guarded_arithmetic_bundle_row]])
    let guarded_arithmetic_bundle_running_task = guarded_arithmetic_bundle_forwarded
    let guarded_arithmetic_bundle_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_bundle_running_task, worker(8)],
        },
        tail: guarded_arithmetic_bundle_tail_tasks[1],
    }
    let guarded_arithmetic_bundle_root = guarded_arithmetic_bundle_env.bundle.tasks
    let guarded_arithmetic_bundle_tasks = guarded_arithmetic_bundle_root
    let guarded_arithmetic_bundle_bundled = guarded_arithmetic_bundle_tasks[0]
    let guarded_arithmetic_bundle_ready = forward(guarded_arithmetic_bundle_bundled)
    let guarded_arithmetic_bundle_second = await guarded_arithmetic_bundle_ready
    let guarded_arithmetic_bundle_extra = await guarded_arithmetic_bundle_env.bundle.tasks[1]
    let guarded_arithmetic_bundle_tail = await guarded_arithmetic_bundle_env.tail

    let guarded_arithmetic_queued_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_slots = [guarded_arithmetic_queued_row, guarded_arithmetic_queued_row]
    let guarded_arithmetic_queued_slot_alias = guarded_arithmetic_queued_slots
    var guarded_arithmetic_queued_pending = Pending {
        tasks: [worker(2), worker(7)],
    }
    let guarded_arithmetic_queued_alias = guarded_arithmetic_queued_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_first =
            await guarded_arithmetic_queued_alias[guarded_arithmetic_queued_slot_alias[guarded_arithmetic_queued_row]]
        guarded_arithmetic_queued_pending.tasks[guarded_arithmetic_queued_slots[guarded_arithmetic_queued_row]] =
            worker(guarded_arithmetic_queued_first + 6)
    }
    let guarded_arithmetic_queued_tail_tasks = guarded_arithmetic_queued_pending.tasks
    let guarded_arithmetic_queued_forwarded =
        forward(guarded_arithmetic_queued_alias[guarded_arithmetic_queued_slot_alias[guarded_arithmetic_queued_row]])
    let guarded_arithmetic_queued_running_task = guarded_arithmetic_queued_forwarded
    let guarded_arithmetic_queued_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_running_task, worker(10)],
        },
        tail: guarded_arithmetic_queued_tail_tasks[1],
    }
    let guarded_arithmetic_queued_tasks = guarded_arithmetic_queued_env.bundle.tasks
    let guarded_arithmetic_queued = guarded_arithmetic_queued_tasks[0]
    let guarded_arithmetic_queued_ready = forward(guarded_arithmetic_queued)
    let guarded_arithmetic_queued_second = await guarded_arithmetic_queued_ready
    let guarded_arithmetic_queued_extra = await guarded_arithmetic_queued_env.bundle.tasks[1]
    let guarded_arithmetic_queued_tail = await guarded_arithmetic_queued_env.tail

    let guarded_arithmetic_queued_root_inline_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_root_inline_slots = [guarded_arithmetic_queued_root_inline_row, guarded_arithmetic_queued_root_inline_row]
    let guarded_arithmetic_queued_root_inline_slot_alias = guarded_arithmetic_queued_root_inline_slots
    var guarded_arithmetic_queued_root_inline_pending = Pending {
        tasks: [worker(2), worker(11)],
    }
    let guarded_arithmetic_queued_root_inline_alias = guarded_arithmetic_queued_root_inline_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_root_inline_first =
            await guarded_arithmetic_queued_root_inline_alias[guarded_arithmetic_queued_root_inline_slot_alias[guarded_arithmetic_queued_root_inline_row]]
        guarded_arithmetic_queued_root_inline_pending.tasks[guarded_arithmetic_queued_root_inline_slots[guarded_arithmetic_queued_root_inline_row]] =
            worker(guarded_arithmetic_queued_root_inline_first + 9)
    }
    let guarded_arithmetic_queued_root_inline_tail_tasks = guarded_arithmetic_queued_root_inline_pending.tasks
    let guarded_arithmetic_queued_root_inline_forwarded =
        forward(guarded_arithmetic_queued_root_inline_alias[guarded_arithmetic_queued_root_inline_slot_alias[guarded_arithmetic_queued_root_inline_row]])
    let guarded_arithmetic_queued_root_inline_running_task = guarded_arithmetic_queued_root_inline_forwarded
    let guarded_arithmetic_queued_root_inline_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_root_inline_running_task, worker(16)],
        },
        tail: guarded_arithmetic_queued_root_inline_tail_tasks[1],
    }
    let guarded_arithmetic_queued_root_inline_tasks = guarded_arithmetic_queued_root_inline_env.bundle.tasks
    let guarded_arithmetic_queued_root_inline_second =
        await forward(guarded_arithmetic_queued_root_inline_tasks[0])
    let guarded_arithmetic_queued_root_inline_extra =
        await guarded_arithmetic_queued_root_inline_env.bundle.tasks[1]
    let guarded_arithmetic_queued_root_inline_tail = await guarded_arithmetic_queued_root_inline_env.tail

    let guarded_arithmetic_queued_root_alias_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_root_alias_slots = [guarded_arithmetic_queued_root_alias_row, guarded_arithmetic_queued_root_alias_row]
    let guarded_arithmetic_queued_root_alias_slot_alias = guarded_arithmetic_queued_root_alias_slots
    var guarded_arithmetic_queued_root_alias_pending = Pending {
        tasks: [worker(2), worker(12)],
    }
    let guarded_arithmetic_queued_root_alias_alias = guarded_arithmetic_queued_root_alias_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_root_alias_first =
            await guarded_arithmetic_queued_root_alias_alias[guarded_arithmetic_queued_root_alias_slot_alias[guarded_arithmetic_queued_root_alias_row]]
        guarded_arithmetic_queued_root_alias_pending.tasks[guarded_arithmetic_queued_root_alias_slots[guarded_arithmetic_queued_root_alias_row]] =
            worker(guarded_arithmetic_queued_root_alias_first + 10)
    }
    let guarded_arithmetic_queued_root_alias_tail_tasks = guarded_arithmetic_queued_root_alias_pending.tasks
    let guarded_arithmetic_queued_root_alias_forwarded =
        forward(guarded_arithmetic_queued_root_alias_alias[guarded_arithmetic_queued_root_alias_slot_alias[guarded_arithmetic_queued_root_alias_row]])
    let guarded_arithmetic_queued_root_alias_running_task = guarded_arithmetic_queued_root_alias_forwarded
    let guarded_arithmetic_queued_root_alias_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_root_alias_running_task, worker(18)],
        },
        tail: guarded_arithmetic_queued_root_alias_tail_tasks[1],
    }
    let guarded_arithmetic_queued_root_alias_root = guarded_arithmetic_queued_root_alias_env.bundle.tasks
    let guarded_arithmetic_queued_root_alias_tasks = guarded_arithmetic_queued_root_alias_root
    let guarded_arithmetic_queued_root_alias = guarded_arithmetic_queued_root_alias_tasks[0]
    let guarded_arithmetic_queued_root_alias_ready = forward(guarded_arithmetic_queued_root_alias)
    let guarded_arithmetic_queued_root_alias_second = await guarded_arithmetic_queued_root_alias_ready
    let guarded_arithmetic_queued_root_alias_extra =
        await guarded_arithmetic_queued_root_alias_env.bundle.tasks[1]
    let guarded_arithmetic_queued_root_alias_tail = await guarded_arithmetic_queued_root_alias_env.tail

    let guarded_arithmetic_queued_root_alias_inline_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_root_alias_inline_slots = [guarded_arithmetic_queued_root_alias_inline_row, guarded_arithmetic_queued_root_alias_inline_row]
    let guarded_arithmetic_queued_root_alias_inline_slot_alias = guarded_arithmetic_queued_root_alias_inline_slots
    var guarded_arithmetic_queued_root_alias_inline_pending = Pending {
        tasks: [worker(2), worker(14)],
    }
    let guarded_arithmetic_queued_root_alias_inline_alias = guarded_arithmetic_queued_root_alias_inline_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_root_alias_inline_first =
            await guarded_arithmetic_queued_root_alias_inline_alias[guarded_arithmetic_queued_root_alias_inline_slot_alias[guarded_arithmetic_queued_root_alias_inline_row]]
        guarded_arithmetic_queued_root_alias_inline_pending.tasks[guarded_arithmetic_queued_root_alias_inline_slots[guarded_arithmetic_queued_root_alias_inline_row]] =
            worker(guarded_arithmetic_queued_root_alias_inline_first + 12)
    }
    let guarded_arithmetic_queued_root_alias_inline_tail_tasks =
        guarded_arithmetic_queued_root_alias_inline_pending.tasks
    let guarded_arithmetic_queued_root_alias_inline_forwarded =
        forward(guarded_arithmetic_queued_root_alias_inline_alias[guarded_arithmetic_queued_root_alias_inline_slot_alias[guarded_arithmetic_queued_root_alias_inline_row]])
    let guarded_arithmetic_queued_root_alias_inline_running_task =
        guarded_arithmetic_queued_root_alias_inline_forwarded
    let guarded_arithmetic_queued_root_alias_inline_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_root_alias_inline_running_task, worker(22)],
        },
        tail: guarded_arithmetic_queued_root_alias_inline_tail_tasks[1],
    }
    let guarded_arithmetic_queued_root_alias_inline_root =
        guarded_arithmetic_queued_root_alias_inline_env.bundle.tasks
    let guarded_arithmetic_queued_root_alias_inline_tasks =
        guarded_arithmetic_queued_root_alias_inline_root
    let guarded_arithmetic_queued_root_alias_inline =
        guarded_arithmetic_queued_root_alias_inline_tasks[0]
    let guarded_arithmetic_queued_root_alias_inline_second =
        await forward(guarded_arithmetic_queued_root_alias_inline)
    let guarded_arithmetic_queued_root_alias_inline_extra =
        await guarded_arithmetic_queued_root_alias_inline_env.bundle.tasks[1]
    let guarded_arithmetic_queued_root_alias_inline_tail =
        await guarded_arithmetic_queued_root_alias_inline_env.tail

    let guarded_arithmetic_queued_root_chain_inline_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_root_chain_inline_slots = [guarded_arithmetic_queued_root_chain_inline_row, guarded_arithmetic_queued_root_chain_inline_row]
    let guarded_arithmetic_queued_root_chain_inline_slot_alias = guarded_arithmetic_queued_root_chain_inline_slots
    var guarded_arithmetic_queued_root_chain_inline_pending = Pending {
        tasks: [worker(2), worker(15)],
    }
    let guarded_arithmetic_queued_root_chain_inline_alias =
        guarded_arithmetic_queued_root_chain_inline_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_root_chain_inline_first =
            await guarded_arithmetic_queued_root_chain_inline_alias[guarded_arithmetic_queued_root_chain_inline_slot_alias[guarded_arithmetic_queued_root_chain_inline_row]]
        guarded_arithmetic_queued_root_chain_inline_pending.tasks[guarded_arithmetic_queued_root_chain_inline_slots[guarded_arithmetic_queued_root_chain_inline_row]] =
            worker(guarded_arithmetic_queued_root_chain_inline_first + 13)
    }
    let guarded_arithmetic_queued_root_chain_inline_tail_tasks =
        guarded_arithmetic_queued_root_chain_inline_pending.tasks
    let guarded_arithmetic_queued_root_chain_inline_forwarded =
        forward(guarded_arithmetic_queued_root_chain_inline_alias[guarded_arithmetic_queued_root_chain_inline_slot_alias[guarded_arithmetic_queued_root_chain_inline_row]])
    let guarded_arithmetic_queued_root_chain_inline_running_task =
        guarded_arithmetic_queued_root_chain_inline_forwarded
    let guarded_arithmetic_queued_root_chain_inline_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_root_chain_inline_running_task, worker(24)],
        },
        tail: guarded_arithmetic_queued_root_chain_inline_tail_tasks[1],
    }
    let guarded_arithmetic_queued_root_chain_inline_root =
        guarded_arithmetic_queued_root_chain_inline_env.bundle.tasks
    let guarded_arithmetic_queued_root_chain_inline_alias_root =
        guarded_arithmetic_queued_root_chain_inline_root
    let guarded_arithmetic_queued_root_chain_inline_tasks =
        guarded_arithmetic_queued_root_chain_inline_alias_root
    let guarded_arithmetic_queued_root_chain_inline =
        guarded_arithmetic_queued_root_chain_inline_tasks[0]
    let guarded_arithmetic_queued_root_chain_inline_second =
        await forward(guarded_arithmetic_queued_root_chain_inline)
    let guarded_arithmetic_queued_root_chain_inline_extra =
        await guarded_arithmetic_queued_root_chain_inline_env.bundle.tasks[1]
    let guarded_arithmetic_queued_root_chain_inline_tail =
        await guarded_arithmetic_queued_root_chain_inline_env.tail

    let guarded_arithmetic_queued_root_chain_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_root_chain_slots = [guarded_arithmetic_queued_root_chain_row, guarded_arithmetic_queued_root_chain_row]
    let guarded_arithmetic_queued_root_chain_slot_alias = guarded_arithmetic_queued_root_chain_slots
    var guarded_arithmetic_queued_root_chain_pending = Pending {
        tasks: [worker(2), worker(13)],
    }
    let guarded_arithmetic_queued_root_chain_alias = guarded_arithmetic_queued_root_chain_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_root_chain_first =
            await guarded_arithmetic_queued_root_chain_alias[guarded_arithmetic_queued_root_chain_slot_alias[guarded_arithmetic_queued_root_chain_row]]
        guarded_arithmetic_queued_root_chain_pending.tasks[guarded_arithmetic_queued_root_chain_slots[guarded_arithmetic_queued_root_chain_row]] =
            worker(guarded_arithmetic_queued_root_chain_first + 11)
    }
    let guarded_arithmetic_queued_root_chain_tail_tasks = guarded_arithmetic_queued_root_chain_pending.tasks
    let guarded_arithmetic_queued_root_chain_forwarded =
        forward(guarded_arithmetic_queued_root_chain_alias[guarded_arithmetic_queued_root_chain_slot_alias[guarded_arithmetic_queued_root_chain_row]])
    let guarded_arithmetic_queued_root_chain_running_task = guarded_arithmetic_queued_root_chain_forwarded
    let guarded_arithmetic_queued_root_chain_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_root_chain_running_task, worker(20)],
        },
        tail: guarded_arithmetic_queued_root_chain_tail_tasks[1],
    }
    let guarded_arithmetic_queued_root_chain_root = guarded_arithmetic_queued_root_chain_env.bundle.tasks
    let guarded_arithmetic_queued_root_chain_alias_root = guarded_arithmetic_queued_root_chain_root
    let guarded_arithmetic_queued_root_chain_tasks = guarded_arithmetic_queued_root_chain_alias_root
    let guarded_arithmetic_queued_root_chain = guarded_arithmetic_queued_root_chain_tasks[0]
    let guarded_arithmetic_queued_root_chain_ready = forward(guarded_arithmetic_queued_root_chain)
    let guarded_arithmetic_queued_root_chain_second = await guarded_arithmetic_queued_root_chain_ready
    let guarded_arithmetic_queued_root_chain_extra =
        await guarded_arithmetic_queued_root_chain_env.bundle.tasks[1]
    let guarded_arithmetic_queued_root_chain_tail = await guarded_arithmetic_queued_root_chain_env.tail

    let guarded_arithmetic_queued_local_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_local_slots = [guarded_arithmetic_queued_local_row, guarded_arithmetic_queued_local_row]
    let guarded_arithmetic_queued_local_slot_alias = guarded_arithmetic_queued_local_slots
    var guarded_arithmetic_queued_local_pending = Pending {
        tasks: [worker(2), worker(8)],
    }
    let guarded_arithmetic_queued_local_alias = guarded_arithmetic_queued_local_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_local_first =
            await guarded_arithmetic_queued_local_alias[guarded_arithmetic_queued_local_slot_alias[guarded_arithmetic_queued_local_row]]
        guarded_arithmetic_queued_local_pending.tasks[guarded_arithmetic_queued_local_slots[guarded_arithmetic_queued_local_row]] =
            worker(guarded_arithmetic_queued_local_first + 7)
    }
    let guarded_arithmetic_queued_local_tail_tasks = guarded_arithmetic_queued_local_pending.tasks
    let guarded_arithmetic_queued_local_forwarded =
        forward(guarded_arithmetic_queued_local_alias[guarded_arithmetic_queued_local_slot_alias[guarded_arithmetic_queued_local_row]])
    let guarded_arithmetic_queued_local_running_task = guarded_arithmetic_queued_local_forwarded
    let guarded_arithmetic_queued_local_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_local_running_task, worker(12)],
        },
        tail: guarded_arithmetic_queued_local_tail_tasks[1],
    }
    let guarded_arithmetic_queued_local_root = guarded_arithmetic_queued_local_env.bundle.tasks
    let guarded_arithmetic_queued_local_alias_root = guarded_arithmetic_queued_local_root
    let guarded_arithmetic_queued_local_tasks = guarded_arithmetic_queued_local_alias_root
    let guarded_arithmetic_queued_local = guarded_arithmetic_queued_local_tasks[0]
    let guarded_arithmetic_queued_local_alias_task = guarded_arithmetic_queued_local
    let guarded_arithmetic_queued_local_final = guarded_arithmetic_queued_local_alias_task
    let guarded_arithmetic_queued_local_ready = forward(guarded_arithmetic_queued_local_final)
    let guarded_arithmetic_queued_local_second = await guarded_arithmetic_queued_local_ready
    let guarded_arithmetic_queued_local_extra = await guarded_arithmetic_queued_local_env.bundle.tasks[1]
    let guarded_arithmetic_queued_local_tail = await guarded_arithmetic_queued_local_env.tail

    let guarded_arithmetic_queued_local_inline_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_queued_local_inline_slots = [guarded_arithmetic_queued_local_inline_row, guarded_arithmetic_queued_local_inline_row]
    let guarded_arithmetic_queued_local_inline_slot_alias = guarded_arithmetic_queued_local_inline_slots
    var guarded_arithmetic_queued_local_inline_pending = Pending {
        tasks: [worker(2), worker(9)],
    }
    let guarded_arithmetic_queued_local_inline_alias = guarded_arithmetic_queued_local_inline_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_queued_local_inline_first =
            await guarded_arithmetic_queued_local_inline_alias[guarded_arithmetic_queued_local_inline_slot_alias[guarded_arithmetic_queued_local_inline_row]]
        guarded_arithmetic_queued_local_inline_pending.tasks[guarded_arithmetic_queued_local_inline_slots[guarded_arithmetic_queued_local_inline_row]] =
            worker(guarded_arithmetic_queued_local_inline_first + 8)
    }
    let guarded_arithmetic_queued_local_inline_tail_tasks = guarded_arithmetic_queued_local_inline_pending.tasks
    let guarded_arithmetic_queued_local_inline_forwarded =
        forward(guarded_arithmetic_queued_local_inline_alias[guarded_arithmetic_queued_local_inline_slot_alias[guarded_arithmetic_queued_local_inline_row]])
    let guarded_arithmetic_queued_local_inline_running_task = guarded_arithmetic_queued_local_inline_forwarded
    let guarded_arithmetic_queued_local_inline_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_queued_local_inline_running_task, worker(14)],
        },
        tail: guarded_arithmetic_queued_local_inline_tail_tasks[1],
    }
    let guarded_arithmetic_queued_local_inline_root = guarded_arithmetic_queued_local_inline_env.bundle.tasks
    let guarded_arithmetic_queued_local_inline_alias_root = guarded_arithmetic_queued_local_inline_root
    let guarded_arithmetic_queued_local_inline_tasks = guarded_arithmetic_queued_local_inline_alias_root
    let guarded_arithmetic_queued_local_inline = guarded_arithmetic_queued_local_inline_tasks[0]
    let guarded_arithmetic_queued_local_inline_alias_task = guarded_arithmetic_queued_local_inline
    let guarded_arithmetic_queued_local_inline_final = guarded_arithmetic_queued_local_inline_alias_task
    let guarded_arithmetic_queued_local_inline_second = await forward(guarded_arithmetic_queued_local_inline_final)
    let guarded_arithmetic_queued_local_inline_extra = await guarded_arithmetic_queued_local_inline_env.bundle.tasks[1]
    let guarded_arithmetic_queued_local_inline_tail = await guarded_arithmetic_queued_local_inline_env.tail

    let guarded_arithmetic_chain_row = ARITH_SLOT_ALIAS.value
    let guarded_arithmetic_chain_slots = [guarded_arithmetic_chain_row, guarded_arithmetic_chain_row]
    let guarded_arithmetic_chain_slot_alias = guarded_arithmetic_chain_slots
    var guarded_arithmetic_chain_pending = Pending {
        tasks: [worker(2), worker(6)],
    }
    let guarded_arithmetic_chain_alias = guarded_arithmetic_chain_pending.tasks
    if ARITH_SLOT_ALIAS.value == 0 {
        let guarded_arithmetic_chain_first =
            await guarded_arithmetic_chain_alias[guarded_arithmetic_chain_slot_alias[guarded_arithmetic_chain_row]]
        guarded_arithmetic_chain_pending.tasks[guarded_arithmetic_chain_slots[guarded_arithmetic_chain_row]] =
            worker(guarded_arithmetic_chain_first + 5)
    }
    let guarded_arithmetic_chain_tail_tasks = guarded_arithmetic_chain_pending.tasks
    let guarded_arithmetic_chain_forwarded =
        forward(guarded_arithmetic_chain_alias[guarded_arithmetic_chain_slot_alias[guarded_arithmetic_chain_row]])
    let guarded_arithmetic_chain_running_task = guarded_arithmetic_chain_forwarded
    let guarded_arithmetic_chain_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [guarded_arithmetic_chain_running_task, worker(9)],
        },
        tail: guarded_arithmetic_chain_tail_tasks[1],
    }
    let guarded_arithmetic_chain_root = guarded_arithmetic_chain_env.bundle.tasks
    let guarded_arithmetic_chain_alias_root = guarded_arithmetic_chain_root
    let guarded_arithmetic_chain_tasks = guarded_arithmetic_chain_alias_root
    let guarded_arithmetic_chain_bundled = guarded_arithmetic_chain_tasks[0]
    let guarded_arithmetic_chain_ready = forward(guarded_arithmetic_chain_bundled)
    let guarded_arithmetic_chain_second = await guarded_arithmetic_chain_ready
    let guarded_arithmetic_chain_extra = await guarded_arithmetic_chain_env.bundle.tasks[1]
    let guarded_arithmetic_chain_tail = await guarded_arithmetic_chain_env.tail

    return tuple_second
        + tuple_extra
        + tuple_tail
        + struct_second
        + struct_extra
        + struct_tail
        + nested_second
        + nested_extra
        + nested_tail
        + arithmetic_const_second
        + arithmetic_const_extra
        + arithmetic_const_tail
        + arithmetic_static_second
        + arithmetic_static_extra
        + arithmetic_static_tail
        + arithmetic_const_composed_second
        + arithmetic_const_composed_extra
        + arithmetic_const_composed_tail
        + arithmetic_static_composed_second
        + arithmetic_static_composed_extra
        + arithmetic_static_composed_tail
        + guarded_arithmetic_const_second
        + guarded_arithmetic_const_extra
        + guarded_arithmetic_const_tail
        + guarded_arithmetic_static_second
        + guarded_arithmetic_static_extra
        + guarded_arithmetic_static_tail
        + guarded_arithmetic_bundle_second
        + guarded_arithmetic_bundle_extra
        + guarded_arithmetic_bundle_tail
        + guarded_arithmetic_queued_second
        + guarded_arithmetic_queued_extra
        + guarded_arithmetic_queued_tail
        + guarded_arithmetic_queued_root_inline_second
        + guarded_arithmetic_queued_root_inline_extra
        + guarded_arithmetic_queued_root_inline_tail
        + guarded_arithmetic_queued_root_alias_second
        + guarded_arithmetic_queued_root_alias_extra
        + guarded_arithmetic_queued_root_alias_tail
        + guarded_arithmetic_queued_root_alias_inline_second
        + guarded_arithmetic_queued_root_alias_inline_extra
        + guarded_arithmetic_queued_root_alias_inline_tail
        + guarded_arithmetic_queued_root_chain_inline_second
        + guarded_arithmetic_queued_root_chain_inline_extra
        + guarded_arithmetic_queued_root_chain_inline_tail
        + guarded_arithmetic_queued_root_chain_second
        + guarded_arithmetic_queued_root_chain_extra
        + guarded_arithmetic_queued_root_chain_tail
        + guarded_arithmetic_queued_local_second
        + guarded_arithmetic_queued_local_extra
        + guarded_arithmetic_queued_local_tail
        + guarded_arithmetic_queued_local_inline_second
        + guarded_arithmetic_queued_local_inline_extra
        + guarded_arithmetic_queued_local_inline_tail
        + guarded_arithmetic_chain_second
        + guarded_arithmetic_chain_extra
        + guarded_arithmetic_chain_tail
}
