fn choose(value: Int, enabled: Bool) -> Int {
    return match value {
        1 if enabled => 42,
        2 => 0,
    }
}

fn main() -> Int {
    return choose(1, true)
}
