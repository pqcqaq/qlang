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

pub fn first3_array[T](values: [T; 3]) -> T {
    return first_array(values)
}

pub fn first4_array[T](values: [T; 4]) -> T {
    return first_array(values)
}

pub fn first5_array[T](values: [T; 5]) -> T {
    return first_array(values)
}

pub fn last3_array[T](values: [T; 3]) -> T {
    return last_array(values)
}

pub fn last4_array[T](values: [T; 4]) -> T {
    return last_array(values)
}

pub fn last5_array[T](values: [T; 5]) -> T {
    return last_array(values)
}

pub fn at3_array_or[T](values: [T; 3], index: Int, fallback: T) -> T {
    return at_array_or(values, index, fallback)
}

pub fn at4_array_or[T](values: [T; 4], index: Int, fallback: T) -> T {
    return at_array_or(values, index, fallback)
}

pub fn at5_array_or[T](values: [T; 5], index: Int, fallback: T) -> T {
    return at_array_or(values, index, fallback)
}

pub fn reverse3_array[T](values: [T; 3]) -> [T; 3] {
    return [values[2], values[1], values[0]]
}

pub fn reverse4_array[T](values: [T; 4]) -> [T; 4] {
    return [values[3], values[2], values[1], values[0]]
}

pub fn reverse5_array[T](values: [T; 5]) -> [T; 5] {
    return [values[4], values[3], values[2], values[1], values[0]]
}

pub fn repeat3_array[T](value: T) -> [T; 3] {
    return [value, value, value]
}

pub fn repeat4_array[T](value: T) -> [T; 4] {
    return [value, value, value, value]
}

pub fn repeat5_array[T](value: T) -> [T; 5] {
    return [value, value, value, value, value]
}

pub fn contains3_array[T](values: [T; 3], needle: T) -> Bool {
    return contains_array(values, needle)
}

pub fn contains4_array[T](values: [T; 4], needle: T) -> Bool {
    return contains_array(values, needle)
}

pub fn contains5_array[T](values: [T; 5], needle: T) -> Bool {
    return contains_array(values, needle)
}

pub fn count3_array[T](values: [T; 3], needle: T) -> Int {
    return count_array(values, needle)
}

pub fn count4_array[T](values: [T; 4], needle: T) -> Int {
    return count_array(values, needle)
}

pub fn count5_array[T](values: [T; 5], needle: T) -> Int {
    return count_array(values, needle)
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

pub fn sum3_int_array(values: [Int; 3]) -> Int {
    return sum_int_array(values)
}

pub fn sum4_int_array(values: [Int; 4]) -> Int {
    return sum_int_array(values)
}

pub fn sum5_int_array(values: [Int; 5]) -> Int {
    return sum_int_array(values)
}

pub fn product3_int_array(values: [Int; 3]) -> Int {
    return product_int_array(values)
}

pub fn product4_int_array(values: [Int; 4]) -> Int {
    return product_int_array(values)
}

pub fn product5_int_array(values: [Int; 5]) -> Int {
    return product_int_array(values)
}

pub fn max3_int_array(values: [Int; 3]) -> Int {
    return max_int_array(values)
}

pub fn max4_int_array(values: [Int; 4]) -> Int {
    return max_int_array(values)
}

pub fn max5_int_array(values: [Int; 5]) -> Int {
    return max_int_array(values)
}

pub fn min3_int_array(values: [Int; 3]) -> Int {
    return min_int_array(values)
}

pub fn min4_int_array(values: [Int; 4]) -> Int {
    return min_int_array(values)
}

pub fn min5_int_array(values: [Int; 5]) -> Int {
    return min_int_array(values)
}

pub fn all3_bool_array(values: [Bool; 3]) -> Bool {
    return all_bool_array(values)
}

pub fn all4_bool_array(values: [Bool; 4]) -> Bool {
    return all_bool_array(values)
}

pub fn all5_bool_array(values: [Bool; 5]) -> Bool {
    return all_bool_array(values)
}

pub fn any3_bool_array(values: [Bool; 3]) -> Bool {
    return any_bool_array(values)
}

pub fn any4_bool_array(values: [Bool; 4]) -> Bool {
    return any_bool_array(values)
}

pub fn any5_bool_array(values: [Bool; 5]) -> Bool {
    return any_bool_array(values)
}

pub fn none3_bool_array(values: [Bool; 3]) -> Bool {
    return none_bool_array(values)
}

pub fn none4_bool_array(values: [Bool; 4]) -> Bool {
    return none_bool_array(values)
}

pub fn none5_bool_array(values: [Bool; 5]) -> Bool {
    return none_bool_array(values)
}
