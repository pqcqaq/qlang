extern "c" fn choose() -> Bool
extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let left_run = (value: Int) => value + target
    let right_run = (value: Int) => value + target + 1
    let left_check = (value: Int) => value == target
    let right_check = (value: Int) => value + 1 == target + 1
    var run_alias = left_run
    var check_alias = left_check

    defer {
        let chosen_run = if choose() { run_alias = right_run } else { left_run }
        let rebound_run = chosen_run
        rebound_run(1)

        let chosen_check = if choose() { check_alias = right_check } else { left_check }
        let rebound_check = chosen_check
        if rebound_check(42) {
            keep()
        }
    }

    return 0
}
