extern "c" fn first()
extern "c" fn second()
extern "c" fn third()

fn enabled() -> Bool {
    return true
}

fn score_first() -> Int {
    first()
    return 1
}

fn score_second() -> Int {
    second()
    return 2
}

fn score_third() -> Int {
    third()
    return 3
}

const ALPHA: String = "alpha"
static BETA: String = "beta"
const GAMMA: String = "gamma"
static DELTA: String = "delta"

const PICK_FIRST: () -> Int = score_first
static PICK_SECOND: () -> Int = score_second
const PICK_THIRD: () -> Int = score_third

fn main() -> Int {
    let cleanup_value = "beta"
    let call_value = "delta"
    defer match cleanup_value {
        ALPHA if enabled() => first(),
        BETA => second(),
        _ => third(),
    }
    defer (match call_value {
        GAMMA if enabled() => PICK_FIRST,
        DELTA => PICK_SECOND,
        _ => PICK_THIRD,
    })()
    return 0
}
