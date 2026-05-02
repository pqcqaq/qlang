package std.array

pub fn first3_array[T](values: [T; 3]) -> T {
    return values[0]
}

pub fn first4_array[T](values: [T; 4]) -> T {
    return values[0]
}

pub fn first5_array[T](values: [T; 5]) -> T {
    return values[0]
}

pub fn last3_array[T](values: [T; 3]) -> T {
    return values[2]
}

pub fn last4_array[T](values: [T; 4]) -> T {
    return values[3]
}

pub fn last5_array[T](values: [T; 5]) -> T {
    return values[4]
}

pub fn at3_array_or[T](values: [T; 3], index: Int, fallback: T) -> T {
    if index == 0 {
        return values[0]
    }
    if index == 1 {
        return values[1]
    }
    if index == 2 {
        return values[2]
    }
    return fallback
}

pub fn at4_array_or[T](values: [T; 4], index: Int, fallback: T) -> T {
    if index == 0 {
        return values[0]
    }
    if index == 1 {
        return values[1]
    }
    if index == 2 {
        return values[2]
    }
    if index == 3 {
        return values[3]
    }
    return fallback
}

pub fn at5_array_or[T](values: [T; 5], index: Int, fallback: T) -> T {
    if index == 0 {
        return values[0]
    }
    if index == 1 {
        return values[1]
    }
    if index == 2 {
        return values[2]
    }
    if index == 3 {
        return values[3]
    }
    if index == 4 {
        return values[4]
    }
    return fallback
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
    return values[0] == needle || values[1] == needle || values[2] == needle
}

pub fn contains4_array[T](values: [T; 4], needle: T) -> Bool {
    return values[0] == needle || values[1] == needle || values[2] == needle || values[3] == needle
}

pub fn contains5_array[T](values: [T; 5], needle: T) -> Bool {
    return values[0] == needle || values[1] == needle || values[2] == needle || values[3] == needle || values[4] == needle
}

pub fn count3_array[T](values: [T; 3], needle: T) -> Int {
    let first = if values[0] == needle {
        1
    } else {
        0
    }
    let second = if values[1] == needle {
        1
    } else {
        0
    }
    let third = if values[2] == needle {
        1
    } else {
        0
    }
    return first + second + third
}

pub fn count4_array[T](values: [T; 4], needle: T) -> Int {
    let first = if values[0] == needle {
        1
    } else {
        0
    }
    let second = if values[1] == needle {
        1
    } else {
        0
    }
    let third = if values[2] == needle {
        1
    } else {
        0
    }
    let fourth = if values[3] == needle {
        1
    } else {
        0
    }
    return first + second + third + fourth
}

pub fn count5_array[T](values: [T; 5], needle: T) -> Int {
    let first = if values[0] == needle {
        1
    } else {
        0
    }
    let second = if values[1] == needle {
        1
    } else {
        0
    }
    let third = if values[2] == needle {
        1
    } else {
        0
    }
    let fourth = if values[3] == needle {
        1
    } else {
        0
    }
    let fifth = if values[4] == needle {
        1
    } else {
        0
    }
    return first + second + third + fourth + fifth
}

pub fn sum3_int_array(values: [Int; 3]) -> Int {
    return values[0] + values[1] + values[2]
}

pub fn sum4_int_array(values: [Int; 4]) -> Int {
    return values[0] + values[1] + values[2] + values[3]
}

pub fn sum5_int_array(values: [Int; 5]) -> Int {
    return values[0] + values[1] + values[2] + values[3] + values[4]
}

pub fn product3_int_array(values: [Int; 3]) -> Int {
    return values[0] * values[1] * values[2]
}

pub fn product4_int_array(values: [Int; 4]) -> Int {
    return values[0] * values[1] * values[2] * values[3]
}

pub fn product5_int_array(values: [Int; 5]) -> Int {
    return values[0] * values[1] * values[2] * values[3] * values[4]
}

pub fn max3_int_array(values: [Int; 3]) -> Int {
    let first = if values[0] > values[1] {
        values[0]
    } else {
        values[1]
    }
    if first > values[2] {
        return first
    }
    return values[2]
}

pub fn max4_int_array(values: [Int; 4]) -> Int {
    let first = max3_int_array([values[0], values[1], values[2]])
    if first > values[3] {
        return first
    }
    return values[3]
}

pub fn max5_int_array(values: [Int; 5]) -> Int {
    let first = max4_int_array([values[0], values[1], values[2], values[3]])
    if first > values[4] {
        return first
    }
    return values[4]
}

pub fn min3_int_array(values: [Int; 3]) -> Int {
    let first = if values[0] < values[1] {
        values[0]
    } else {
        values[1]
    }
    if first < values[2] {
        return first
    }
    return values[2]
}

pub fn min4_int_array(values: [Int; 4]) -> Int {
    let first = min3_int_array([values[0], values[1], values[2]])
    if first < values[3] {
        return first
    }
    return values[3]
}

pub fn min5_int_array(values: [Int; 5]) -> Int {
    let first = min4_int_array([values[0], values[1], values[2], values[3]])
    if first < values[4] {
        return first
    }
    return values[4]
}

pub fn all3_bool_array(values: [Bool; 3]) -> Bool {
    return values[0] && values[1] && values[2]
}

pub fn all4_bool_array(values: [Bool; 4]) -> Bool {
    return all3_bool_array([values[0], values[1], values[2]]) && values[3]
}

pub fn all5_bool_array(values: [Bool; 5]) -> Bool {
    return all4_bool_array([values[0], values[1], values[2], values[3]]) && values[4]
}

pub fn any3_bool_array(values: [Bool; 3]) -> Bool {
    return values[0] || values[1] || values[2]
}

pub fn any4_bool_array(values: [Bool; 4]) -> Bool {
    return any3_bool_array([values[0], values[1], values[2]]) || values[3]
}

pub fn any5_bool_array(values: [Bool; 5]) -> Bool {
    return any4_bool_array([values[0], values[1], values[2], values[3]]) || values[4]
}

pub fn none3_bool_array(values: [Bool; 3]) -> Bool {
    return !any3_bool_array(values)
}

pub fn none4_bool_array(values: [Bool; 4]) -> Bool {
    return !any4_bool_array(values)
}

pub fn none5_bool_array(values: [Bool; 5]) -> Bool {
    return !any5_bool_array(values)
}
