extern "c" fn keep()

fn main() -> Int {
    let branch = true
    let target = 42
    let check = (value: Int) => value == target
    var check_alias = check
    let run = (value: Int) => value + target
    var run_alias = run
    defer (if branch { run_alias = run } else { run })(1)
    defer if (match branch {
        true => check_alias = check,
        false => check,
    })(42) {
        keep()
    }
    return 0
}
