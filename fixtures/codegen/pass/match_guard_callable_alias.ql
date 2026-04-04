use READY as ready
use SHIFT as offset

fn enabled() -> Bool {
    return true
}

fn shift(value: Int, delta: Int) -> Int {
    return value + delta
}

const READY: () -> Bool = enabled
const SHIFT: (Int, Int) -> Int = shift

fn main() -> Int {
    let first = match true {
        true if ready() => 10,
        false => 0,
    }
    let second = match 20 {
        current if offset(current, 2) == 22 => 32,
        _ => 0,
    }
    return first + second
}
