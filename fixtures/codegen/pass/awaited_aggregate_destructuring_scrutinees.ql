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

fn pair_value() -> (Int, Int) {
    return (3, 4)
}

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
    let direct = match pair_value() {
        (left, right) if left < right => left + right,
        _ => 0,
    }
    match await (if branch { pair_alias } else { pair_const_alias })(20) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }
    match await (match branch { true => state_const_alias, false => state_alias })(13) {
        State { slot: Slot { value } } if value == 13 => sink(value),
        _ => sink(0),
    }
    return direct
}
