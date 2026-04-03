fn choose(flag: Bool) -> Int {
    return match flag {
        true if flag => 42,
        false => 0,
    }
}

fn main() -> Int {
    return choose(true)
}
