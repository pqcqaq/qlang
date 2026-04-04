struct State {
    current: Int,
    pair: (Int, Int),
    values: [Int; 3],
}

fn main() -> Int {
    var total = 1
    var index = 1
    var state = State {
        current: 2,
        pair: (3, 4),
        values: [5, 6, 7],
    }
    defer {
        total = 8;
        state.current = total + 1;
        state.pair[0] = state.current + 1;
        state.values[index] = state.pair[0] + 1;
    }
    return total + state.current + state.pair[0] + state.values[1]
}
