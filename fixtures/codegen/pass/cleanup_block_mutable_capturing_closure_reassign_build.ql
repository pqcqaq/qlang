extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let check = (value: Int) => value == target
    let base = 40
    let run = (value: Int) => value + base + 1
    defer {
        var alias_run = run
        alias_run = run;
        alias_run(1)
        var alias_check = check
        alias_check = check;
        if alias_check(42) {
            keep()
        }
    }
    return 0
}
