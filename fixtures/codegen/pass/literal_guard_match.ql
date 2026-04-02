fn main() -> Int {
    let value = 2
    return match value {
        1 if false => 10,
        2 if true => 20,
        other if true => other,
    }
}
