package std.core

pub fn max_int(left: Int, right: Int) -> Int {
    if left > right {
        return left
    }
    return right
}

pub fn min_int(left: Int, right: Int) -> Int {
    if left < right {
        return left
    }
    return right
}

pub fn clamp_int(value: Int, low: Int, high: Int) -> Int {
    if value < low {
        return low
    }
    if value > high {
        return high
    }
    return value
}

pub fn abs_int(value: Int) -> Int {
    if value < 0 {
        return 0 - value
    }
    return value
}

pub fn bool_to_int(value: Bool) -> Int {
    if value {
        return 1
    }
    return 0
}
