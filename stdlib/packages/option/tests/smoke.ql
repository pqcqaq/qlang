use std.option.Option as Option
use std.option.is_none as is_none
use std.option.is_some as is_some
use std.option.none_option as option_none
use std.option.or_option as or_option
use std.option.some as some
use std.option.unwrap_or as unwrap_or

fn check_int(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn check_bool(actual: Bool, expected: Bool) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn sum_statuses[N](statuses: [Int; N]) -> Int {
    var total = 0
    for status in statuses {
        total = total + status
    }
    return total
}

fn generic_option_status(some_value: Option[Int], none_value: Option[Int]) -> Int {
    let some_status = match some_value {
        Option.Some(inner) => inner,
        Option.None => 0,
    }
    let none_status = match none_value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
    return some_status + none_status
}

fn main() -> Int {
    let int_some: Option[Int] = some(7)
    let int_none: Option[Int] = option_none()
    let int_fallback: Option[Int] = some(9)
    let int_present: Option[Int] = some(13)
    let bool_some: Option[Bool] = some(true)
    let bool_none: Option[Bool] = option_none()
    let bool_fallback: Option[Bool] = some(false)
    let direct_some: Option[Int] = Option.Some(15)
    let direct_none: Option[Int] = Option.None
    let int_status = sum_statuses([check_bool(is_some(int_some), true), check_bool(is_none(int_none), true), check_int(unwrap_or(int_some, 3), 7), check_int(unwrap_or(int_none, 3), 3), check_int(unwrap_or(or_option(int_none, int_fallback), 0), 9), check_int(unwrap_or(or_option(int_present, int_fallback), 0), 13)])
    let bool_status = sum_statuses([check_bool(is_some(bool_some), true), check_bool(is_none(bool_none), true), check_bool(unwrap_or(bool_some, false), true), check_bool(unwrap_or(bool_none, true), true), check_bool(unwrap_or(or_option(bool_none, bool_fallback), true), false), 0])
    let constructor_status = sum_statuses([check_bool(is_some(direct_some), true), check_bool(is_none(direct_none), true), check_int(unwrap_or(direct_some, 0), 15), check_int(unwrap_or(direct_none, 3), 3), 0, 0])
    return int_status + bool_status + constructor_status + check_int(generic_option_status(Option.Some(7), Option.None), 7)
}
