struct Pair {
    left: Int,
    right: Int,
}

struct Outer {
    pair: Pair,
    values: [Int; 2],
}

struct Wrap {
    values: [Int; 0],
}

fn pick_pair(outer: Outer) -> Int {
    return outer.pair.right
}

fn pick_tuple(pair: (Bool, Int)) -> Int {
    if pair[0] {
        return pair[1]
    }
    return 0
}

fn write_at(index: Int) -> Int {
    var values = [1, 2, 3]
    values[index] = 9
    return values[index]
}

fn write_cell(row: Int, col: Int) -> Int {
    var matrix = [[1, 2, 3], [4, 5, 6]]
    matrix[row][col] = 8
    return matrix[row][col]
}

fn zero_values() -> [Int; 0] {
    return []
}

fn zero_wrap() -> Wrap {
    return Wrap { values: [] }
}

fn nested_zero() -> [[Int; 0]; 1] {
    return [[]]
}

fn take_zero(values: [Int; 0]) -> Int {
    return 1
}

fn main() -> Int {
    let outer = Outer {
        pair: Pair { left: 1, right: 2 },
        values: [3, 4],
    }
    let tuple = (true, 5)
    let wrap = zero_wrap()
    let nested = nested_zero()

    return pick_pair(outer)
        + outer.values[1]  
        + pick_tuple(tuple)
        + write_at(1)
        + write_cell(1, 2)
        + take_zero(zero_values())
        + take_zero(wrap.values)
        + take_zero(nested[0])
        + take_zero([])
}
