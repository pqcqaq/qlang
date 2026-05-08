package std.result

use std.option.Option as Option

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn ok[T, E](value: T) -> Result[T, E] {
    return Result.Ok(value)
}

pub fn err[T, E](error: E) -> Result[T, E] {
    return Result.Err(error)
}

pub fn is_ok[T, E](value: Result[T, E]) -> Bool {
    return match value {
        Result.Ok(_) => true,
        Result.Err(_) => false,
    }
}

pub fn is_err[T, E](value: Result[T, E]) -> Bool {
    return match value {
        Result.Ok(_) => false,
        Result.Err(_) => true,
    }
}

pub fn unwrap_result_or[T, E](value: Result[T, E], fallback: T) -> T {
    return match value {
        Result.Ok(inner) => inner,
        Result.Err(_) => fallback,
    }
}

pub fn or_result[T, E](value: Result[T, E], fallback: Result[T, E]) -> Result[T, E] {
    return match value {
        Result.Ok(inner) => Result.Ok(inner),
        Result.Err(_) => fallback,
    }
}

pub fn error_or[T, E](value: Result[T, E], fallback: E) -> E {
    return match value {
        Result.Ok(_) => fallback,
        Result.Err(error) => error,
    }
}

pub fn ok_or[T, E](value: Option[T], error: E) -> Result[T, E] {
    return match value {
        Option.Some(inner) => Result.Ok(inner),
        Option.None => Result.Err(error),
    }
}

pub fn to_option[T, E](value: Result[T, E]) -> Option[T] {
    return match value {
        Result.Ok(inner) => Option.Some(inner),
        Result.Err(_) => Option.None,
    }
}

pub fn error_to_option[T, E](value: Result[T, E]) -> Option[E] {
    return match value {
        Result.Ok(_) => Option.None,
        Result.Err(error) => Option.Some(error),
    }
}
