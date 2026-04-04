use load_state as async_alias
use LOAD as async_const_alias

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

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
    let branch = true
    return match 1 {
        1 if (await (if branch { async_alias } else { async_const_alias })(13)).slot.value == 13 => 10,
        1 if (await (match branch { true => async_const_alias, false => async_alias })(14)).slot.value == 14 => 20,
        _ => 0,
    }
}
