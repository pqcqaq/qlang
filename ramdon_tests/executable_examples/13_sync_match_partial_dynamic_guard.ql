fn choose(flag: Bool, enabled: Bool) -> Int {
    return match flag {
        true if enabled => 42,
        false => 0,
    }
}

fn main() -> Int {
    return choose(true, true)
}
