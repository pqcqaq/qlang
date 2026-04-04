use load_state as async_alias
use LOAD as async_const_alias

struct State {
    value: Int,
}

async fn load_state(value: Int) -> State {
    return State {
        value: value,
    }
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[1] == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
    let branch = true
    return match 1 {
        1 if State { value: (await (if branch { async_alias } else { async_const_alias })(22)).value }.value == 22 => 10,
        1 if matches((0, (await (match branch { true => async_const_alias, false => async_alias })(23)).value), 23) => 20,
        1 if contains([0, (await (if branch { async_alias } else { async_const_alias })(24)).value, 2], 24) => 30,
        _ => 0,
    }
}
