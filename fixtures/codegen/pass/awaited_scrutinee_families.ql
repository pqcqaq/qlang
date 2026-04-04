use load_state as async_alias
use LOAD as async_const_alias
use matches as helper_alias

struct Slot {
    value: Int,
}

struct State {
    slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
    return State { slot: Slot { value: value } }
}

fn matches(expected: Int, value: Int) -> Bool {
    return expected == value
}

fn wrap(state: State) -> State {
    return state
}

fn offset(value: Int) -> Int {
    return value - 11
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
    let branch = true
    match helper_alias(13, (await (if branch { async_alias } else { async_const_alias })(13)).slot.value) {
        true => sink(1),
        false => sink(0),
    }
    match State { slot: Slot { value: (await (match branch { true => async_const_alias, false => async_alias })(14)).slot.value } }.slot.value {
        14 => sink(2),
        _ => sink(3),
    }
    match wrap(await (if branch { async_alias } else { async_const_alias })(15)).slot.value {
        15 => sink(4),
        _ => sink(5),
    }
    match [wrap(await (match branch { true => async_const_alias, false => async_alias })(17)).slot.value, 0][offset(11)] {
        17 => sink(6),
        _ => sink(7),
    }
    return 0
}
