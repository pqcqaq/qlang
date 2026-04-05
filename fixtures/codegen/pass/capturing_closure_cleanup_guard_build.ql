fn main() -> Int {
    let target = 42
    let check = (value: Int) => value == target
    let base = 40
    let run = (value: Int) => value + base + 1
    defer run(1)
    return match 42 {
        current if check(current) => 1,
        _ => 0,
    }
}
