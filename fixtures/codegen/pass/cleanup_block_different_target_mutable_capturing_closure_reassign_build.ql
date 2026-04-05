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
    var match_direct_run = left_run
    match choose() {
        true => {
            match_direct_run = right_run;
        },
        false => {},
    }
    let chosen_run = {
        var alias = left_run
        if choose() {
            alias = right_run;
        };
        alias
    }
    let match_chosen_run = {
        var alias = left_run
        match choose() {
            true => {
                alias = right_run;
            },
            false => {},
        };
        alias
    }
    var assignment_run = left_run
    let direct_control_flow_assignment_run =
        (if choose() { assignment_run = right_run } else { left_run })(9)
    var binding_run = left_run
    let chosen_control_flow_run = match choose() {
        true => binding_run = right_run,
        false => left_run,
    }
    let rebound_control_flow_run = chosen_control_flow_run

    var direct_check = left_check
    if choose() {
        direct_check = right_check;
    }
    var match_direct_check = left_check
    match choose() {
        true => {
            match_direct_check = right_check;
        },
        false => {},
    }
    let chosen_check = {
        var alias = left_check
        if choose() {
            alias = right_check;
        };
        alias
    }
    let match_chosen_check = {
        var alias = left_check
        match choose() {
            true => {
                alias = right_check;
            },
            false => {},
        };
        alias
    }

    defer direct_run(1)
    defer chosen_run(2)
    defer match_direct_run(3)
    defer match_chosen_run(4)
    defer if direct_check(42) {
        keep()
    }
    defer if chosen_check(42) {
        keep()
    }
    defer if match_direct_check(42) {
        keep()
    }
    defer if match_chosen_check(42) {
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

    let ordinary = direct_run(5)
        + chosen_run(6)
        + match_direct_run(7)
        + match_chosen_run(8)
        + direct_control_flow_assignment_run
        + rebound_control_flow_run(10)
    let direct_guarded = match 42 {
        current if direct_check(current) => 1,
        current if chosen_check(current) => 2,
        _ => 0,
    }
    let match_direct_guarded = match 42 {
        current if match_direct_check(current) => 3,
        _ => 0,
    }
    let match_bound_guarded = match 42 {
        current if match_chosen_check(current) => 4,
        _ => 0,
    }
    let guarded = direct_guarded + match_direct_guarded + match_bound_guarded
    return ordinary + guarded
}
