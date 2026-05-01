package std.result

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
