use matches as helper_alias

struct State {
    value: Int,
}

extern "c" fn sink(value: Int)

async fn worker(value: Int) -> State {
    return State {
        value: value + 1,
    }
}

fn matches(expected: Int, state: State) -> Bool {
    return state.value == expected
}

async fn main() -> Int {
    let branch = true
    let which = 1
    let first = spawn worker(41)
    let second = spawn worker(1)
    let left = () => first
    let right = () => second

    defer if helper_alias(42, await (if branch { left } else { right })()) {
        sink(1);
    }

    defer match State { value: (await (match which { 1 => left, _ => right })()).value }.value {
        42 => sink(2),
        _ => sink(3),
    }
    return 0
}
