struct Payload {
    values: [Int; 2],
}

fn array_values(base: Int) -> [Int; 2] {
    return [base, base]
}

fn tuple_values(base: Int) -> (Int, Int) {
    return (base, base + 1)
}

fn make_payload(base: Int) -> Payload {
    return Payload {
        values: [base, base + 1],
    }
}

fn main() -> Int {
    var total = 0
    for value in array_values(10) {
        total = total + value
    }
    for value in tuple_values(7) {
        total = total + value
    }
    for value in make_payload(3).values {
        total = total + value
    }
    return total
}
