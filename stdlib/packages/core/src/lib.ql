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

pub fn max3_int(first: Int, second: Int, third: Int) -> Int {
    return max_int(max_int(first, second), third)
}

pub fn min3_int(first: Int, second: Int, third: Int) -> Int {
    return min_int(min_int(first, second), third)
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

pub fn clamp_min_int(value: Int, low: Int) -> Int {
    if value < low {
        return low
    }
    return value
}

pub fn clamp_max_int(value: Int, high: Int) -> Int {
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

pub fn abs_diff_int(left: Int, right: Int) -> Int {
    if left > right {
        return left - right
    }
    return right - left
}

pub fn sign_int(value: Int) -> Int {
    if value < 0 {
        return 0 - 1
    }
    if value > 0 {
        return 1
    }
    return 0
}

pub fn compare_int(left: Int, right: Int) -> Int {
    if left < right {
        return 0 - 1
    }
    if left > right {
        return 1
    }
    return 0
}

pub fn median3_int(first: Int, second: Int, third: Int) -> Int {
    if is_ascending_int(first, second, third) || is_ascending_int(third, second, first) {
        return second
    }
    if is_ascending_int(second, first, third) || is_ascending_int(third, first, second) {
        return first
    }
    return third
}

pub fn is_zero_int(value: Int) -> Bool {
    return value == 0
}

pub fn is_nonzero_int(value: Int) -> Bool {
    return value != 0
}

pub fn is_positive_int(value: Int) -> Bool {
    return value > 0
}

pub fn is_nonnegative_int(value: Int) -> Bool {
    return value >= 0
}

pub fn is_negative_int(value: Int) -> Bool {
    return value < 0
}

pub fn is_nonpositive_int(value: Int) -> Bool {
    return value <= 0
}

pub fn is_even_int(value: Int) -> Bool {
    return value % 2 == 0
}

pub fn is_odd_int(value: Int) -> Bool {
    return value % 2 != 0
}

pub fn in_range_int(value: Int, low: Int, high: Int) -> Bool {
    return value >= low && value <= high
}

pub fn in_exclusive_range_int(value: Int, low: Int, high: Int) -> Bool {
    return value > low && value < high
}

pub fn is_ascending_int(first: Int, second: Int, third: Int) -> Bool {
    return first <= second && second <= third
}

pub fn is_strictly_ascending_int(first: Int, second: Int, third: Int) -> Bool {
    return first < second && second < third
}

pub fn is_divisible_by_int(value: Int, divisor: Int) -> Bool {
    return divisor != 0 && value % divisor == 0
}

pub fn is_within_int(value: Int, target: Int, tolerance: Int) -> Bool {
    return tolerance >= 0 && abs_diff_int(value, target) <= tolerance
}

pub fn bool_to_int(value: Bool) -> Int {
    if value {
        return 1
    }
    return 0
}

pub fn not_bool(value: Bool) -> Bool {
    return !value
}

pub fn and_bool(left: Bool, right: Bool) -> Bool {
    return left && right
}

pub fn or_bool(left: Bool, right: Bool) -> Bool {
    return left || right
}

pub fn xor_bool(left: Bool, right: Bool) -> Bool {
    return left != right
}

pub fn implies_bool(left: Bool, right: Bool) -> Bool {
    return !left || right
}
