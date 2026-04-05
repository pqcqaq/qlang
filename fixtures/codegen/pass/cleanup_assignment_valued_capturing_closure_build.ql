extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let check = (value: Int) => value == target
    var check_alias = check
    let run = (value: Int) => value + target
    var run_alias = run
    defer (run_alias = run)(1)
    defer if (check_alias = check)(42) {
        keep()
    }
    return 0
}
