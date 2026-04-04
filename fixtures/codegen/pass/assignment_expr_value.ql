struct State {
    current: Int,
    values: [Int; 3],
}

fn forward(value: Int) -> Int {
    return value
}

fn main() -> Int {
    var index = 1
    var state = State {
        current: 2,
        values: [3, 4, 5],
    }
    let first = forward(state.current = 6)
    let second = {
        state.values[index] = state.current + 1
    }
    return first + second + state.values[1]
}
