fn enabled() -> Bool {
    return true
}

const ALPHA: String = "alpha"
static BETA: String = "beta"

fn main() -> Int {
    let offset = 40
    let branch = false
    let left = (value: Int) => value + offset
    let right = (value: Int) => value + offset + 10
    let fallback = (value: Int) => value + offset + 20
    let direct_key = "alpha"
    let binding_key = "beta"
    let chosen = match binding_key {
        ALPHA if branch => left,
        BETA if enabled() => right,
        _ => fallback,
    }
    let alias = chosen
    return (match direct_key {
        ALPHA if enabled() => left,
        BETA if branch => right,
        _ => fallback,
    })(1) + alias(2)
}
