struct Pair {
    value: Int,
    values: [Int; 2],
}

fn main() -> Int {
    var pair = Pair { value: 1, values: [2, 3] }
    var total = 1
    let first = total = 4
    let second = pair.value = first + 2
    let third = pair.values[1] = second + 3
    return first + second + third + total + pair.value + pair.values[1]
}
