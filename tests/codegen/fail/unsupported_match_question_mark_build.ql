fn helper() -> Int {
    let flag = true
    let enabled = false
    return match flag {
        true if enabled => 1,
        false => 0,
    }
}

fn main() -> Int {
    return helper()?
}
