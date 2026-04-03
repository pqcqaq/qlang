use SELECTED_INDEX as SELECTED_INDEX_ALIAS
use SELECTED_SLOT as SELECTED_SLOT_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0
const NEXT: Int = 1
const SELECTED_INDEX: Int = if NEXT == 1 { 0 } else { 1 }
const STEP: Int = 1
const ARITH_INDEX: Int = STEP - 1

static MATCH_KEY: Int = 1
static SELECTED_SLOT: Slot = match MATCH_KEY {
    1 => Slot { value: 0 },
    _ => Slot { value: 1 },
}

async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return 0
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(5), worker(8)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let second = await alias[slot.value]
    let tail = await pending.tasks[1]

    var const_pending = Pending {
        tasks: [worker(6), worker(9)],
    }
    let const_alias = const_pending.tasks
    let const_first = await const_alias[INDEX]
    const_pending.tasks[0] = worker(const_first + 2)
    let const_second = await const_alias[INDEX]
    let const_tail = await const_pending.tasks[1]

    let row = choose()
    var composed = [worker(1), worker(2)]
    let slots = [row, row]
    let composed_first = await composed[slots[row]]
    composed[slots[row]] = worker(composed_first + 1)
    let composed_final = await composed[slots[row]]

    var alias_sourced = [worker(3), worker(4)]
    let more_slots = [row, row]
    let slot_alias = more_slots
    let alias_sourced_first = await alias_sourced[slot_alias[row]]
    alias_sourced[more_slots[row]] = worker(alias_sourced_first + 1)
    let alias_sourced_final = await alias_sourced[slot_alias[row]]

    var inline_if_tasks = [worker(2), worker(7)]
    let inline_if_first = await inline_if_tasks[if true { 0 } else { 1 }]
    inline_if_tasks[if true { 0 } else { 1 }] = worker(inline_if_first + 4)
    let inline_if_second = await inline_if_tasks[0]

    var inline_match_tasks = [worker(4), worker(9)]
    let inline_match_first = await inline_match_tasks[match 1 {
        1 => 0,
        _ => 1,
    }]
    inline_match_tasks[match 1 {
        1 => 0,
        _ => 1,
    }] = worker(inline_match_first + 5)
    let inline_match_second = await inline_match_tasks[0]

    var const_if_tasks = [worker(6), worker(10)]
    let const_if_first = await const_if_tasks[SELECTED_INDEX]
    const_if_tasks[0] = worker(const_if_first + 3)
    let const_if_second = await const_if_tasks[SELECTED_INDEX]

    var static_match_pending = Pending {
        tasks: [worker(3), worker(11)],
    }
    let static_match_alias = static_match_pending.tasks
    let static_match_first = await static_match_alias[SELECTED_SLOT.value]
    static_match_pending.tasks[0] = worker(static_match_first + 6)
    let static_match_second = await static_match_alias[SELECTED_SLOT.value]

    var arithmetic_const_tasks = [worker(7), worker(12)]
    let arithmetic_const_first = await arithmetic_const_tasks[ARITH_INDEX]
    arithmetic_const_tasks[0] = worker(arithmetic_const_first + 2)
    let arithmetic_const_second = await arithmetic_const_tasks[ARITH_INDEX]

    var arithmetic_projected_tasks = [worker(8), worker(13)]
    let arithmetic_slot = Slot { value: 3 - 3 }
    let arithmetic_projected_first = await arithmetic_projected_tasks[arithmetic_slot.value]
    arithmetic_projected_tasks[0] = worker(arithmetic_projected_first + 4)
    let arithmetic_projected_second = await arithmetic_projected_tasks[1 - 1]

    var aliased_branch_const_tasks = [worker(9), worker(14)]
    let aliased_branch_const_first = await aliased_branch_const_tasks[SELECTED_INDEX_ALIAS]
    aliased_branch_const_tasks[0] = worker(aliased_branch_const_first + 1)
    let aliased_branch_const_second = await aliased_branch_const_tasks[SELECTED_INDEX_ALIAS]

    var aliased_branch_static_pending = Pending {
        tasks: [worker(4), worker(15)],
    }
    let aliased_branch_static_alias = aliased_branch_static_pending.tasks
    let aliased_branch_static_first = await aliased_branch_static_alias[SELECTED_SLOT_ALIAS.value]
    aliased_branch_static_pending.tasks[0] = worker(aliased_branch_static_first + 5)
    let aliased_branch_static_second = await aliased_branch_static_alias[SELECTED_SLOT_ALIAS.value]

    return first
        + second
        + tail
        + const_first
        + const_second
        + const_tail
        + composed_first
        + composed_final
        + alias_sourced_first
        + alias_sourced_final
        + inline_if_first
        + inline_if_second
        + inline_match_first
        + inline_match_second
        + const_if_first
        + const_if_second
        + static_match_first
        + static_match_second
        + arithmetic_const_first
        + arithmetic_const_second
        + arithmetic_projected_first
        + arithmetic_projected_second
        + aliased_branch_const_first
        + aliased_branch_const_second
        + aliased_branch_static_first
        + aliased_branch_static_second
}
