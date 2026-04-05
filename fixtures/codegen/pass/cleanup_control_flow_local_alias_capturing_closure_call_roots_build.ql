extern "c" fn keep()

fn main() -> Int {
    let branch = true
    let target = 42
    let check = (value: Int) => value == target
    let run = (value: Int) => value + target
    defer (if branch {
        let alias = run
        alias
    } else {
        run
    })(1)
    defer if (match branch {
        true => {
            var alias = check
            alias = check;
            alias
        },
        false => check,
    })(42) {
        keep()
    }
    return 0
}
