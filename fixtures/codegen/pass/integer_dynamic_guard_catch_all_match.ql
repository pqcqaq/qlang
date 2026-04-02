fn main() -> Int {
    let value = 3
    let enabled = true
    return match value {
        1 => 10,
        other if enabled => other,
        _ => 0,
    }
}
