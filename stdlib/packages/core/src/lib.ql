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

pub fn max_ints[N](values: [Int; N]) -> Int {
    var selected = values[0]
    for value in values {
        if value > selected {
            selected = value
        }
    }
    return selected
}

pub fn min_ints[N](values: [Int; N]) -> Int {
    var selected = values[0]
    for value in values {
        if value < selected {
            selected = value
        }
    }
    return selected
}

pub fn sum_ints[N](values: [Int; N]) -> Int {
    var total = 0
    for value in values {
        total = total + value
    }
    return total
}

pub fn product_ints[N](values: [Int; N]) -> Int {
    var total = 1
    for value in values {
        total = total * value
    }
    return total
}

pub fn average_ints[N](values: [Int; N]) -> Int {
    if N == 0 {
        return 0
    }
    return sum_ints(values) / N
}

pub fn max3_int(first: Int, second: Int, third: Int) -> Int {
    return max_ints([first, second, third])
}

pub fn max4_int(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return max_ints([first, second, third, fourth])
}

pub fn max5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    return max_ints([first, second, third, fourth, fifth])
}

pub fn min3_int(first: Int, second: Int, third: Int) -> Int {
    return min_ints([first, second, third])
}

pub fn min4_int(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return min_ints([first, second, third, fourth])
}

pub fn min5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    return min_ints([first, second, third, fourth, fifth])
}

pub fn sum3_int(first: Int, second: Int, third: Int) -> Int {
    return sum_ints([first, second, third])
}

pub fn sum4_int(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return sum_ints([first, second, third, fourth])
}

pub fn sum5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    return sum_ints([first, second, third, fourth, fifth])
}

pub fn product3_int(first: Int, second: Int, third: Int) -> Int {
    return product_ints([first, second, third])
}

pub fn product4_int(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return product_ints([first, second, third, fourth])
}

pub fn product5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    return product_ints([first, second, third, fourth, fifth])
}

pub fn average2_int(left: Int, right: Int) -> Int {
    return average_ints([left, right])
}

pub fn average3_int(first: Int, second: Int, third: Int) -> Int {
    return average_ints([first, second, third])
}

pub fn average4_int(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return average_ints([first, second, third, fourth])
}

pub fn average5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    return average_ints([first, second, third, fourth, fifth])
}

pub fn quotient_or_zero_int(value: Int, divisor: Int) -> Int {
    if divisor == 0 {
        return 0
    }
    return value / divisor
}

pub fn remainder_or_zero_int(value: Int, divisor: Int) -> Int {
    if divisor == 0 {
        return 0
    }
    return value % divisor
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

pub fn clamp_bounds_int(value: Int, first_bound: Int, second_bound: Int) -> Int {
    return clamp_int(value, min_int(first_bound, second_bound), max_int(first_bound, second_bound))
}

pub fn lower_bound_int(first_bound: Int, second_bound: Int) -> Int {
    return min_int(first_bound, second_bound)
}

pub fn upper_bound_int(first_bound: Int, second_bound: Int) -> Int {
    return max_int(first_bound, second_bound)
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

pub fn range_span_int(first_bound: Int, second_bound: Int) -> Int {
    return abs_diff_int(first_bound, second_bound)
}

pub fn distance_to_range_int(value: Int, low: Int, high: Int) -> Int {
    if value < low {
        return low - value
    }
    if value > high {
        return value - high
    }
    return 0
}

pub fn distance_to_bounds_int(value: Int, first_bound: Int, second_bound: Int) -> Int {
    return distance_to_range_int(value, lower_bound_int(first_bound, second_bound), upper_bound_int(first_bound, second_bound))
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

pub fn in_bounds_int(value: Int, first_bound: Int, second_bound: Int) -> Bool {
    return in_range_int(value, min_int(first_bound, second_bound), max_int(first_bound, second_bound))
}

pub fn in_exclusive_bounds_int(value: Int, first_bound: Int, second_bound: Int) -> Bool {
    return in_exclusive_range_int(value, min_int(first_bound, second_bound), max_int(first_bound, second_bound))
}

pub fn is_outside_range_int(value: Int, low: Int, high: Int) -> Bool {
    return value < low || value > high
}

pub fn is_outside_bounds_int(value: Int, first_bound: Int, second_bound: Int) -> Bool {
    return is_outside_range_int(value, min_int(first_bound, second_bound), max_int(first_bound, second_bound))
}

pub fn is_ascending_int(first: Int, second: Int, third: Int) -> Bool {
    return first <= second && second <= third
}

pub fn is_ascending4_int(first: Int, second: Int, third: Int, fourth: Int) -> Bool {
    return is_ascending_int(first, second, third) && third <= fourth
}

pub fn is_ascending5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Bool {
    return is_ascending4_int(first, second, third, fourth) && fourth <= fifth
}

pub fn is_strictly_ascending_int(first: Int, second: Int, third: Int) -> Bool {
    return first < second && second < third
}

pub fn is_strictly_ascending4_int(first: Int, second: Int, third: Int, fourth: Int) -> Bool {
    return is_strictly_ascending_int(first, second, third) && third < fourth
}

pub fn is_strictly_ascending5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Bool {
    return is_strictly_ascending4_int(first, second, third, fourth) && fourth < fifth
}

pub fn is_descending_int(first: Int, second: Int, third: Int) -> Bool {
    return first >= second && second >= third
}

pub fn is_descending4_int(first: Int, second: Int, third: Int, fourth: Int) -> Bool {
    return is_descending_int(first, second, third) && third >= fourth
}

pub fn is_descending5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Bool {
    return is_descending4_int(first, second, third, fourth) && fourth >= fifth
}

pub fn is_strictly_descending_int(first: Int, second: Int, third: Int) -> Bool {
    return first > second && second > third
}

pub fn is_strictly_descending4_int(first: Int, second: Int, third: Int, fourth: Int) -> Bool {
    return is_strictly_descending_int(first, second, third) && third > fourth
}

pub fn is_strictly_descending5_int(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Bool {
    return is_strictly_descending4_int(first, second, third, fourth) && fourth > fifth
}

pub fn is_divisible_by_int(value: Int, divisor: Int) -> Bool {
    return divisor != 0 && value % divisor == 0
}

pub fn has_remainder_int(value: Int, divisor: Int) -> Bool {
    return divisor != 0 && value % divisor != 0
}

pub fn is_factor_of_int(factor: Int, value: Int) -> Bool {
    return is_divisible_by_int(value, factor)
}

pub fn is_within_int(value: Int, target: Int, tolerance: Int) -> Bool {
    return tolerance >= 0 && abs_diff_int(value, target) <= tolerance
}

pub fn is_not_within_int(value: Int, target: Int, tolerance: Int) -> Bool {
    return !is_within_int(value, target, tolerance)
}

pub fn bool_to_int(value: Bool) -> Int {
    if value {
        return 1
    }
    return 0
}

pub fn all_bools[N](values: [Bool; N]) -> Bool {
    for value in values {
        if !value {
            return false
        }
    }
    return true
}

pub fn any_bools[N](values: [Bool; N]) -> Bool {
    for value in values {
        if value {
            return true
        }
    }
    return false
}

pub fn none_bools[N](values: [Bool; N]) -> Bool {
    return !any_bools(values)
}

pub fn all3_bool(first: Bool, second: Bool, third: Bool) -> Bool {
    return all_bools([first, second, third])
}

pub fn all4_bool(first: Bool, second: Bool, third: Bool, fourth: Bool) -> Bool {
    return all_bools([first, second, third, fourth])
}

pub fn all5_bool(first: Bool, second: Bool, third: Bool, fourth: Bool, fifth: Bool) -> Bool {
    return all_bools([first, second, third, fourth, fifth])
}

pub fn any3_bool(first: Bool, second: Bool, third: Bool) -> Bool {
    return any_bools([first, second, third])
}

pub fn any4_bool(first: Bool, second: Bool, third: Bool, fourth: Bool) -> Bool {
    return any_bools([first, second, third, fourth])
}

pub fn any5_bool(first: Bool, second: Bool, third: Bool, fourth: Bool, fifth: Bool) -> Bool {
    return any_bools([first, second, third, fourth, fifth])
}

pub fn none3_bool(first: Bool, second: Bool, third: Bool) -> Bool {
    return none_bools([first, second, third])
}

pub fn none4_bool(first: Bool, second: Bool, third: Bool, fourth: Bool) -> Bool {
    return none_bools([first, second, third, fourth])
}

pub fn none5_bool(first: Bool, second: Bool, third: Bool, fourth: Bool, fifth: Bool) -> Bool {
    return none_bools([first, second, third, fourth, fifth])
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
