package std.result

use std.option.BoolOption as BoolOption
use std.option.IntOption as IntOption
use std.option.is_some_bool as option_is_some_bool
use std.option.is_some_int as option_is_some_int
use std.option.none_bool as option_none_bool
use std.option.none_int as option_none_int
use std.option.some_bool as option_some_bool
use std.option.some_int as option_some_int
use std.option.unwrap_or_bool as option_unwrap_or_bool
use std.option.unwrap_or_int as option_unwrap_or_int

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub enum IntResult {
    Ok(Int),
    Err(Int),
}

pub enum BoolResult {
    Ok(Bool),
    Err(Int),
}

pub fn ok_int(value: Int) -> IntResult {
    return IntResult.Ok(value)
}

pub fn err_int(error: Int) -> IntResult {
    return IntResult.Err(error)
}

pub fn is_ok_int(value: IntResult) -> Bool {
    return match value {
        IntResult.Ok(_) => true,
        IntResult.Err(_) => false,
    }
}

pub fn is_err_int(value: IntResult) -> Bool {
    return match value {
        IntResult.Ok(_) => false,
        IntResult.Err(_) => true,
    }
}

pub fn unwrap_result_or_int(value: IntResult, fallback: Int) -> Int {
    return match value {
        IntResult.Ok(inner) => inner,
        IntResult.Err(_) => fallback,
    }
}

pub fn or_result_int(value: IntResult, fallback: IntResult) -> IntResult {
    return match value {
        IntResult.Ok(inner) => IntResult.Ok(inner),
        IntResult.Err(_) => fallback,
    }
}

pub fn error_or_zero_int(value: IntResult) -> Int {
    return match value {
        IntResult.Ok(_) => 0,
        IntResult.Err(error) => error,
    }
}

pub fn error_to_option_int(value: IntResult) -> IntOption {
    return match value {
        IntResult.Ok(_) => option_none_int(),
        IntResult.Err(error) => option_some_int(error),
    }
}

pub fn ok_bool(value: Bool) -> BoolResult {
    return BoolResult.Ok(value)
}

pub fn err_bool(error: Int) -> BoolResult {
    return BoolResult.Err(error)
}

pub fn is_ok_bool(value: BoolResult) -> Bool {
    return match value {
        BoolResult.Ok(_) => true,
        BoolResult.Err(_) => false,
    }
}

pub fn is_err_bool(value: BoolResult) -> Bool {
    return match value {
        BoolResult.Ok(_) => false,
        BoolResult.Err(_) => true,
    }
}

pub fn unwrap_result_or_bool(value: BoolResult, fallback: Bool) -> Bool {
    return match value {
        BoolResult.Ok(inner) => inner,
        BoolResult.Err(_) => fallback,
    }
}

pub fn or_result_bool(value: BoolResult, fallback: BoolResult) -> BoolResult {
    return match value {
        BoolResult.Ok(inner) => BoolResult.Ok(inner),
        BoolResult.Err(_) => fallback,
    }
}

pub fn error_or_zero_bool(value: BoolResult) -> Int {
    return match value {
        BoolResult.Ok(_) => 0,
        BoolResult.Err(error) => error,
    }
}

pub fn error_to_option_bool(value: BoolResult) -> IntOption {
    return match value {
        BoolResult.Ok(_) => option_none_int(),
        BoolResult.Err(error) => option_some_int(error),
    }
}

pub fn ok_or_int(value: IntOption, error: Int) -> IntResult {
    if option_is_some_int(value) {
        return IntResult.Ok(option_unwrap_or_int(value, 0))
    }
    return IntResult.Err(error)
}

pub fn ok_or_bool(value: BoolOption, error: Int) -> BoolResult {
    if option_is_some_bool(value) {
        return BoolResult.Ok(option_unwrap_or_bool(value, false))
    }
    return BoolResult.Err(error)
}

pub fn to_option_int(value: IntResult) -> IntOption {
    return match value {
        IntResult.Ok(inner) => option_some_int(inner),
        IntResult.Err(_) => option_none_int(),
    }
}

pub fn to_option_bool(value: BoolResult) -> BoolOption {
    return match value {
        BoolResult.Ok(inner) => option_some_bool(inner),
        BoolResult.Err(_) => option_none_bool(),
    }
}
