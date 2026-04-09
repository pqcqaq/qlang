const ALPHA: String = "alpha"
static BETA: String = "beta"

fn main() -> Int {
    let offset = 40
    let left = (value: Int) => value + offset
    let right = (value: Int) => value + offset + 10
    let fallback = (value: Int) => value + offset + 20
    let direct_key = "delta"
    let binding_key = "beta"
    let chosen = match binding_key {
        ALPHA => left,
        BETA => right,
        _ => fallback,
    }
    let alias = chosen
    return (match direct_key {
        ALPHA => left,
        BETA => right,
        _ => fallback,
    })(1) + alias(2)
}
