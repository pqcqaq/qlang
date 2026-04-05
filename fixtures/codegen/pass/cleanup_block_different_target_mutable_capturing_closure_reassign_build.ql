extern "c" fn choose() -> Bool
extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let left_run = (value: Int) => value + target
    let right_run = (value: Int) => value + target + 1
    let left_check = (value: Int) => value == target
    let right_check = (value: Int) => value + 1 == target + 1

    var direct_run = left_run
    if choose() {
        direct_run = right_run;
    }
    let chosen_run = {
        var alias = left_run
        if choose() {
            alias = right_run;
        };
        alias
    }

    var direct_check = left_check
    if choose() {
        direct_check = right_check;
    }
    let chosen_check = {
        var alias = left_check
        if choose() {
            alias = right_check;
        };
        alias
    }

    defer direct_run(1)
    defer chosen_run(2)
    defer if direct_check(42) {
        keep()
    }
    defer if chosen_check(42) {
        keep()
    }

    defer {
        var inner_run = left_run
        if choose() {
            inner_run = right_run;
        };
        inner_run(5)
        var inner_check = left_check
        if choose() {
            inner_check = right_check;
        };
        if inner_check(42) {
            keep()
        }
    }

    let ordinary = direct_run(3) + chosen_run(4)
    let guarded = match 42 {
        current if direct_check(current) => 1,
        current if chosen_check(current) => 2,
        _ => 0,
    }
    return ordinary + guarded
}
