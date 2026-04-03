fn enabled() -> Bool {
    return false
}

fn main() -> Int {
    let flag = true
    return match flag {
        true if enabled() => 1,
        false => 0,
    }
}
