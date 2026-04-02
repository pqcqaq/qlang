fn choose(value: Int, enabled: Bool) -> Int {
    return match value {
        1 if enabled => 10,
        2 => 20,
        _ => 0,
    }
}

fn main() -> Int {
    return choose(1, false)
}
