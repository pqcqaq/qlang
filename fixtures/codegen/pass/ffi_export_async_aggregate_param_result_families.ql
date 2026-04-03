struct Pair {
    left: Int,
    right: Int,
}

struct Wrap {
    values: [Int; 0],
}

async fn recursive_params(pair: Pair, values: [Int; 2]) -> Int {
    return pair.left + pair.right + values[0] + values[1]
}

async fn zero_sized_params(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Int {
    let values = await empty_values()
    let wrap = await wrapped()

    let recursive_total = await recursive_params(Pair { left: 1, right: 2 }, [3, 4])
    let zero_sized_total = await zero_sized_params(values, wrap, [[]])

    return recursive_total + zero_sized_total
}

extern "c" pub fn q_export() -> Int {
    return 1
}
