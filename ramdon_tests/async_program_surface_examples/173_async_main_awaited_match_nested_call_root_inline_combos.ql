use bundle as pack
use offset as slot

struct Bundle {
    values: [Int; 3],
}

async fn fetch_value(value: Int) -> Int {
    return value
}

fn bundle(seed: Int) -> Bundle {
    return Bundle {
        values: [seed, seed + 1, seed + 2],
    }
}

fn offset(value: Int) -> Int {
    return value - 2
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[1] == expected
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

async fn main() -> Int {
    let first = await fetch_value(value: 3)
    let from_inline_projection = match first {
        current if [pack(current).values[slot(current)], current + 1, 6][0] == 4 => 10,
        _ => 0,
    }

    let second = await fetch_value(value: 3)
    let from_inline_array_arg = match second {
        current if contains([current, pack(current).values[slot(current)], 9], 4) => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_inline_tuple_arg = match third {
        current if matches((8, pack(current).values[slot(current)]), 4) => 20,
        _ => 0,
    }

    return from_inline_projection + from_inline_array_arg + from_inline_tuple_arg
}
