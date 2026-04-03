struct Slot {
    value: Int,
}

struct Config {
    slot: Slot,
}

struct State {
    ready: Bool,
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

fn main() -> Int {
    let config = Config {
        slot: Slot { value: 3 },
    }
    let first = match true {
        true if enabled(extra: bundle(config.slot.value)[offset(config.slot.value)] == 4, state: state(bundle(config.slot.value)[offset(config.slot.value)] == 4)) => 10,
        false => 0,
    }
    let second = match 3 {
        current if [bundle(config.slot.value)[offset(config.slot.value)], current + 5, 9][0] == 4 => 12,
        _ => 0,
    }
    let third = match 3 {
        current if matches(expected: 4, value: [bundle(config.slot.value)[offset(config.slot.value)], current, 9][0]) => 20,
        _ => 0,
    }
    return first + second + third
}
