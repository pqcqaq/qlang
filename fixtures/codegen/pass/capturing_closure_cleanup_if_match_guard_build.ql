extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let check = (value: Int) => value == target
    defer if check(42) {
        keep()
    }
    defer match 42 {
        current if check(current) => keep(),
        _ => keep(),
    }
    return 0
}
