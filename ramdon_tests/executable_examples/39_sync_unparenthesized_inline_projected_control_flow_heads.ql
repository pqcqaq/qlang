struct FlagState {
    ready: Bool,
}

struct PairPayload {
    values: (Int, Int),
}

fn main() -> Int {
    let from_if = if FlagState { ready: true }.ready {
        10
    } else {
        0
    }
    var from_while = 0
    while FlagState { ready: false }.ready {
        from_while = 100
    }
    let from_match = match PairPayload { values: (20, 22) }.values[1] {
        22 => 32,
        _ => 0,
    }
    return from_if + from_while + from_match
}
