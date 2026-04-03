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

const STEP: Int = 1
const ARITH_INDEX: Int = STEP - 1
static BASE: Int = 2
static ARITH_SLOT: Slot = Slot { value: BASE - 2 }

async fn worker(value: Int) -> Int {
    return value
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
}
