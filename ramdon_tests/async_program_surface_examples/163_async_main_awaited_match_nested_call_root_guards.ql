use bundle as pack
use scalar_matches as check

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

fn slot(value: Int) -> Int {
    return value - 2
}

fn ready(value: Int) -> Bool {
    return value == 4
}

fn scalar_matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

async fn main() -> Int {
    let first = await fetch_value(value: 3)
    let from_projection = match first {
        current if pack(current).values[slot(current)] == 4 => 10,
        _ => 0,
    }

    let second = await fetch_value(value: 3)
    let from_direct_call = match second {
        current if ready(pack(current).values[slot(current)]) => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_guard_call = match third {
        current if check(value: pack(current).values[slot(current)], expected: 4) => 20,
        _ => 0,
    }

    return from_projection + from_direct_call + from_guard_call
}
