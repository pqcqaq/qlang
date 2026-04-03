struct Slot {
    value: Int,
}

struct Config {
    slot: Slot,
}

struct State {
    ready: Bool,
}

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
}

fn state(flag: Bool) -> State {
    return State { ready: flag }
}

fn bundle(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
    return value - 2
}

fn enabled(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

async fn main() -> Int {
    let config = Config {
        slot: Slot { value: 3 },
    }

    let first = await fetch_flag(value: true)
    let from_bool = match first {
        true if enabled(extra: bundle(config.slot.value)[offset(config.slot.value)] == 4, state: state(bundle(config.slot.value)[offset(config.slot.value)] == 4)) => 10,
        false => 0,
    }

    let second = await fetch_value(value: 3)
    let from_inline = match second {
        current if [bundle(config.slot.value)[offset(config.slot.value)], current + 5, 9][0] == 4 => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_guard_call = match third {
        current if matches(expected: 4, value: [bundle(config.slot.value)[offset(config.slot.value)], current, 9][0]) => 20,
        _ => 0,
    }

    return from_bool + from_inline + from_guard_call
}
