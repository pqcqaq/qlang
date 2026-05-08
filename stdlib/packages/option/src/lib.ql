package std.option

pub enum Option[T] {
    Some(T),
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
