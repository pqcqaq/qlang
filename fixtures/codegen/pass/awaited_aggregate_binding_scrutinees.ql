use load_state as state_alias
use load_pair as pair_alias
use LOAD_STATE as state_const_alias
use LOAD_PAIR as pair_const_alias

struct Slot {
    ready: Bool,
    value: Int,
}

struct State {
    slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
    return State { slot: Slot { ready: true, value: value } }
}

async fn load_pair(value: Int) -> (Int, Int) {
    return (value, value + 1)
}

const LOAD_STATE: (Int) -> Task[State] = load_state
const LOAD_PAIR: (Int) -> Task[(Int, Int)] = load_pair

async fn main() -> Int {
    let branch = true
    match await (if branch { state_alias } else { state_const_alias })(13) {
        current => sink(current.slot.value),
    }
    match await (match branch { true => pair_const_alias, false => pair_alias })(20) {
        current => sink(current[0] + current[1]),
    }
    return 0
}
