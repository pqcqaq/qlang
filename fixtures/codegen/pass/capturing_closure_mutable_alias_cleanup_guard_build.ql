extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let check_base = (value: Int) => value == target
    var check = check_base
    let base = 40
    let run_base = (value: Int) => value + base + 1
    var run = run_base
    defer run(1)
    defer if check(42) {
        keep()
    }
    return match 42 {
        current if check(current) => run(1),
        _ => 0,
    }
}
