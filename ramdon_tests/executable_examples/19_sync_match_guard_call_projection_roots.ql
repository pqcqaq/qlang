struct State {
    value: Int,
}

fn pair(value: Int) -> (Int, Int) {
    return (0, value)
}

fn state(value: Int) -> State {
    return State { value: value }
}

fn values(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn main() -> Int {
    let first = match 22 {
        current if pair(current)[1] == 22 => 10,
        _ => 0,
    }
    let second = match 12 {
        current if state(current).value == 12 => 12,
        _ => 0,
    }
    let third = match 3 {
        current if values(current)[1] == 4 => 20,
        _ => 0,
    }
    return first + second + third
}
