fn enabled() -> Bool {
    return false
}

fn helper() -> Int {
    let flag = true
    return match flag {
        true if enabled() => 1,
        false => 0,
    }
}

fn main() -> Int {
    return helper()?
}
