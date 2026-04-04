use load_state as async_alias
use LOAD as async_const_alias

struct State {
    value: Int,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
    return State { value: value }
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
    let branch = true
    defer match (await (if branch { async_alias } else { async_const_alias })(15)).value {
        15 => sink(1),
        _ => sink(0),
    }
    defer match (await (match branch { true => async_const_alias, false => async_alias })(16)).value {
        16 => sink(2),
        _ => sink(3),
    }
    return 0
}
