package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub enum IntOption {
    Some(Int),
    None,
}

pub enum BoolOption {
    Some(Bool),
    None,
}

pub fn some[T](value: T) -> Option[T] {
    return Option.Some(value)
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
}

pub fn is_some[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => true,
        Option.None => false,
    }
}

pub fn is_none[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => false,
        Option.None => true,
    }
}

pub fn unwrap_or[T](value: Option[T], fallback: T) -> T {
    return match value {
        Option.Some(inner) => inner,
        Option.None => fallback,
    }
}

pub fn or_option[T](value: Option[T], fallback: Option[T]) -> Option[T] {
    return match value {
        Option.Some(inner) => Option.Some(inner),
        Option.None => fallback,
    }
}

pub fn some_int(value: Int) -> IntOption {
    return IntOption.Some(value)
}

pub fn none_int() -> IntOption {
    return IntOption.None
}

pub fn is_some_int(value: IntOption) -> Bool {
    return match value {
        IntOption.Some(_) => true,
        IntOption.None => false,
    }
}

pub fn is_none_int(value: IntOption) -> Bool {
    return match value {
        IntOption.Some(_) => false,
        IntOption.None => true,
    }
}

pub fn unwrap_or_int(value: IntOption, fallback: Int) -> Int {
    return match value {
        IntOption.Some(inner) => inner,
        IntOption.None => fallback,
    }
}

pub fn or_int(value: IntOption, fallback: IntOption) -> IntOption {
    return match value {
        IntOption.Some(inner) => IntOption.Some(inner),
        IntOption.None => fallback,
    }
}

pub fn or_option_int(value: IntOption, fallback: IntOption) -> IntOption {
    return or_int(value, fallback)
}

pub fn value_or_zero_int(value: IntOption) -> Int {
    return unwrap_or_int(value, 0)
}

pub fn some_bool(value: Bool) -> BoolOption {
    return BoolOption.Some(value)
}

pub fn none_bool() -> BoolOption {
    return BoolOption.None
}

pub fn is_some_bool(value: BoolOption) -> Bool {
    return match value {
        BoolOption.Some(_) => true,
        BoolOption.None => false,
    }
}

pub fn is_none_bool(value: BoolOption) -> Bool {
    return match value {
        BoolOption.Some(_) => false,
        BoolOption.None => true,
    }
}

pub fn unwrap_or_bool(value: BoolOption, fallback: Bool) -> Bool {
    return match value {
        BoolOption.Some(inner) => inner,
        BoolOption.None => fallback,
    }
}

pub fn or_option_bool(value: BoolOption, fallback: BoolOption) -> BoolOption {
    return match value {
        BoolOption.Some(inner) => BoolOption.Some(inner),
        BoolOption.None => fallback,
    }
}

pub fn value_or_false_bool(value: BoolOption) -> Bool {
    return unwrap_or_bool(value, false)
}

pub fn value_or_true_bool(value: BoolOption) -> Bool {
    return unwrap_or_bool(value, true)
}
