use load_state as async_alias
use LOAD as async_const_alias
use matches as helper_alias

struct State {
    value: Int,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
    return State {
        value: value,
    }
}

fn matches(expected: Int, state: State) -> Bool {
    return state.value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
    let branch = true
    defer match helper_alias(13, await (if branch { async_alias } else { async_const_alias })(13)) {
        true => sink(1),
        false => sink(0),
    }
    defer match State { value: (await (match branch { true => async_const_alias, false => async_alias })(14)).value }.value {
        14 => sink(2),
        _ => sink(3),
    }
    return 0
}
