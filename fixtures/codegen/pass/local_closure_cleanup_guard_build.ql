fn main() -> Int {
    let check = (value: Int) => value == 42
    let run = (value: Int) => value + 1
    defer run(41)
    return match 42 {
        current if check(current) => 1,
        _ => 0,
    }
}
