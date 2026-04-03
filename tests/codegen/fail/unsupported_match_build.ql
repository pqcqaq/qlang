fn main() -> Int {
    let flag = true
    let enabled = false
    return match flag {
        true if enabled => 1,
        false => 0,
    }
}
