struct State {
    value: Int,
}

fn main() -> Int {
    let value = 22
    let first = match value {
        current if (0, current)[1] == 22 => 10,
        _ => 0,
    }
    let second = match value {
        current if State { value: current }.value == 22 => 12,
        _ => 0,
    }
    let third = match 3 {
        current if [current, current + 1, current + 2][1] == 4 => 20,
        _ => 0,
    }
    return first + second + third
}
