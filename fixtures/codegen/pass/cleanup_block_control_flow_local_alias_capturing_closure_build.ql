extern "c" fn keep()

fn main() -> Int {
    let branch = true
    let target = 42
    let check = (value: Int) => value == target
    let run = (value: Int) => value + target
    defer {
        let chosen_run = if branch {
            let alias = run
            alias
        } else {
            run
        }
        let chosen_check = match branch {
            true => {
                var alias = check
                alias = check;
                alias
            },
            false => check,
        }
        chosen_run(1)
        if chosen_check(42) {
            keep()
        }
    }
    return 0
}
