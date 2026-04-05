extern "c" fn keep()

fn main() -> Int {
    let branch = true
    let target = 42
    let check_base = (value: Int) => value == target
    let check_alias = check_base
    let run_base = (value: Int) => value + target
    let run_alias = run_base
    let check = match branch {
        true => check_base,
        false => check_alias,
    }
    let run = if branch { run_base } else { run_alias }
    defer (if branch { run_base } else { run_alias })(1)
    defer if (match branch {
        true => check_base,
        false => check_alias,
    })(42) {
        keep()
    }
    return match 1 {
        current if check(target) => run(current),
        _ => 0,
    }
}
