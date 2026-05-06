package std.array

pub fn first_array[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn last_array[T, N](values: [T; N]) -> T {
    var last = values[0]
    for value in values {
        last = value
    }
    return last
}

pub fn at_array_or[T, N](values: [T; N], index: Int, fallback: T) -> T {
    var current_index = 0
    for value in values {
        if current_index == index {
            return value
        };
        current_index = current_index + 1
    }
    return fallback
}

pub fn contains_array[T, N](values: [T; N], needle: T) -> Bool {
    for value in values {
        if value == needle {
            return true
        }
    }
    return false
}

pub fn count_array[T, N](values: [T; N], needle: T) -> Int {
    var count = 0
    for value in values {
        if value == needle {
            count = count + 1
        }
    }
    return count
}

pub fn len_array[T, N](values: [T; N]) -> Int {
    return N
}

pub fn reverse_array[T, N](values: [T; N]) -> [T; N] {
    var result = values
    var index = 0
    for value in values {
        result[index] = values[N - index - 1];
        index = index + 1
    }
    return result
}

pub fn repeat_array[T, N](value: T) -> [T; N] {
    return [value; N]
}

pub fn sum_int_array[N](values: [Int; N]) -> Int {
    var total = 0
    for value in values {
        total = total + value
    }
    return total
}

pub fn product_int_array[N](values: [Int; N]) -> Int {
    var total = 1
    for value in values {
        total = total * value
    }
    return total
}

pub fn average_int_array[N](values: [Int; N]) -> Int {
    if N == 0 {
        return 0
    }
    return sum_int_array(values) / N
}

pub fn max_int_array[N](values: [Int; N]) -> Int {
    var selected = values[0]
    for value in values {
        if value > selected {
            selected = value
        }
    }
    return selected
}

pub fn min_int_array[N](values: [Int; N]) -> Int {
    var selected = values[0]
    for value in values {
        if value < selected {
            selected = value
        }
    }
    return selected
}

pub fn all_bool_array[N](values: [Bool; N]) -> Bool {
    for value in values {
        if !value {
            return false
        }
    }
    return true
}

pub fn any_bool_array[N](values: [Bool; N]) -> Bool {
    for value in values {
        if value {
            return true
        }
    }
    return false
}

pub fn none_bool_array[N](values: [Bool; N]) -> Bool {
    for value in values {
        if value {
            return false
        }
    }
    return true
}
