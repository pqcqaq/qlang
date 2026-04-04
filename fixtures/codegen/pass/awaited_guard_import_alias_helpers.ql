use load_state as async_alias
use LOAD as async_const_alias
use matches as helper_alias

struct Slot {
    value: Int,
}

struct State {
    slot: Slot,
}

async fn load_state(value: Int) -> State {
    return State {
        slot: Slot { value: value },
    }
}

fn matches(expected: Int, state: State) -> Bool {
    return state.slot.value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
    let branch = true
    return match 1 {
        1 if helper_alias(13, await (if branch { async_alias } else { async_const_alias })(13)) => 10,
        1 if helper_alias(14, await (match branch { true => async_const_alias, false => async_alias })(14)) => 20,
        _ => 0,
    }
}
