use bundle as pack
use offset as slot
use matches as check

fn bundle(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
    return value - 2
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

fn pair(left: Int, right: Int) -> (Int, Int) {
    return (left, right)
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[0] == expected
}

fn main() -> Int {
    let first = match 3 {
        current if [pack(current)[slot(current)], current + 1, 6][0] == 4 => 10,
        _ => 0,
    }
    let second = match 22 {
        current if contains([pack(3)[slot(3)], current, 9], 4) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if check(expected: 4, value: pair(left: pack(current)[slot(current)], right: 8)[0]) => 20,
        _ => 0,
    }
    return first + second + third
}
