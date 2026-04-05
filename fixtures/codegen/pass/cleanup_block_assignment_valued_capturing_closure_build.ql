extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let check = (value: Int) => value == target
    var check_alias = check
    let run = (value: Int) => value + target
    var run_alias = run
    defer {
        let chosen_run = run_alias = run
        let chosen_check = check_alias = check
        chosen_run(1)
        if chosen_check(42) {
            keep()
        }
    }
    return 0
}
