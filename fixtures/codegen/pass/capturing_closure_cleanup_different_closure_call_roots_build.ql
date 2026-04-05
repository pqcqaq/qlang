extern "c" fn keep()

fn main() -> Int {
    let branch = true
    let target = 42
    let left_run = (value: Int) => value + target
    let right_run = (value: Int) => value + target + 1
    let left_check = (value: Int) => value == target
    let right_check = (value: Int) => value + 1 == target + 1

    defer (if branch { left_run } else { right_run })(1)
    defer (match branch {
        true => {
            let alias = left_run
            alias
        },
        false => right_run,
    })(2)

    defer if (if branch { left_check } else { right_check })(42) {
        keep()
    }
    defer if (match branch {
        true => {
            let alias = left_check
            alias
        },
        false => right_check,
    })(42) {
        keep()
    }

    return 0
}
