use shift as offset

fn enabled() -> Bool {
    return true
}

fn shift(value: Int, delta: Int) -> Int {
    return value + delta
}

fn main() -> Int {
    let first = match true {
        true if enabled() => 10,
        false => 0,
    }
    let second = match 20 {
        current if offset(delta: 2, value: current) == 22 => 32,
        _ => 0,
    }
    return first + second
}
