extern "c" fn keep()

fn main() -> Int {
    let target = 42
    let left_run = (value: Int) => value + target
    let right_run = (value: Int) => value + target + 1
    let left_check = (value: Int) => value == target
    let right_check = (value: Int) => value + 1 == target + 1

    defer ({
        var alias = left_run
        alias = right_run;
        alias
    })(1)

    defer if ({
        var alias = left_check
        alias = right_check;
        alias
    })(42) {
        keep()
    }

    defer {
        var inner_run = left_run
        inner_run = right_run;
        inner_run(2)
        var inner_check = left_check
        inner_check = right_check;
        if inner_check(42) {
            keep()
        }
    }

    return 0
}
