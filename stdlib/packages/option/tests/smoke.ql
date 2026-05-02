use std.option.is_none_bool as is_none_bool
use std.option.is_none_int as is_none_int
use std.option.Option as Option
use std.option.is_some_bool as is_some_bool
use std.option.is_some_int as is_some_int
use std.option.none_bool as none_bool
use std.option.none_int as none_int
use std.option.or_option_bool as or_option_bool
use std.option.or_option_int as or_option_int
use std.option.or_int as or_int
use std.option.some_bool as some_bool
use std.option.some_int as some_int
use std.option.unwrap_or_bool as unwrap_or_bool
use std.option.unwrap_or_int as unwrap_or_int
use std.option.value_or_false_bool as value_or_false_bool
use std.option.value_or_true_bool as value_or_true_bool
use std.option.value_or_zero_int as value_or_zero_int

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

fn sum6(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, sixth: Int) -> Int {
    return first + second + third + fourth + fifth + sixth
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
    let int_status = sum6(check_bool(is_some_int(some_int(7)), true), check_bool(is_none_int(none_int()), true), check_int(unwrap_or_int(some_int(7), 3), 7), check_int(unwrap_or_int(none_int(), 3), 3), check_int(value_or_zero_int(none_int()), 0), check_int(unwrap_or_int(or_int(none_int(), some_int(9)), 0), 9))
    let int_alias_status = sum6(check_int(unwrap_or_int(or_option_int(none_int(), some_int(11)), 0), 11), check_int(unwrap_or_int(or_option_int(some_int(13), some_int(11)), 0), 13), 0, 0, 0, 0)
    let bool_status = sum6(check_bool(is_some_bool(some_bool(true)), true), check_bool(is_none_bool(none_bool()), true), check_bool(unwrap_or_bool(some_bool(true), false), true), check_bool(unwrap_or_bool(none_bool(), true), true), check_bool(value_or_false_bool(none_bool()), false), check_bool(value_or_true_bool(none_bool()), true))

    return int_status + int_alias_status + bool_status + check_bool(unwrap_or_bool(or_option_bool(none_bool(), some_bool(false)), true), false) + check_int(generic_option_status(Option.Some(7), Option.None), 7)
}
