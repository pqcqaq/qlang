fn main() -> Int {
    let value = 1
    let enabled = false
    return match value {
        1 if enabled != true => 10,
        _ => 0,
    }
}
