fn choose(value: String, ready: Bool) -> Int {
    return match value {
        "alpha" if ready => 10,
        "beta" => 20,
        _ => 0,
    }
}

fn main() -> Int {
    return choose("beta", false)
}
