struct Pending {
    tuple: (Int, Int),
    array: [Int; 1],
}

fn main() -> Int {
    let pending = Pending {
        tuple: (20, 20),
        array: [2],
    }

    var total = 0
    for value in pending.tuple {
        total = total + value
    }
    for value in pending.array {
        total = total + value
    }
    return total
}
