use bundle as pack
use matches as check

struct Bundle {
    values: [Int; 3],
}

fn bundle(seed: Int) -> Bundle {
    return Bundle { values: [seed, seed + 1, seed + 2] }
}

fn offset(value: Int) -> Int {
    return value - 2
}

fn ready(value: Int) -> Bool {
    return value == 4
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

fn main() -> Int {
    let first = match 3 {
        current if pack(current).values[offset(current)] == 4 => 10,
        _ => 0,
    }
    let second = match 3 {
        current if ready(pack(current).values[offset(current)]) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if check(expected: 4, value: pack(current).values[offset(current)]) => 20,
        _ => 0,
    }
    return first + second + third
}
