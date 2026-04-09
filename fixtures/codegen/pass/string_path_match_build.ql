const ALPHA: String = "alpha"
static BETA: String = "beta"

fn choose(value: String, ready: Bool) -> Int {
    return match value {
        ALPHA if ready => 10,
        BETA => 20,
        _ => 0,
    }
}

fn main() -> Int {
    return choose("beta", false)
}
