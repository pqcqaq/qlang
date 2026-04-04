struct Boxed {
    values: [Int; 3],
}

fn main() -> Int {
    let branch = true
    var boxed = Boxed { values: [0, 0, 0] }
    var total = 0
    for value in ({ let current = Boxed { values: [1, 2, 3] }; current }).values {
        total = total + value
    }
    for value in (boxed = Boxed { values: [4, 5, 6] }).values {
        total = total + value
    }
    for value in (if branch { Boxed { values: [7, 8, 9] } } else { Boxed { values: [10, 11, 12] } }).values {
        total = total + value
    }
    for item in (match branch {
        true => Boxed { values: [13, 14, 15] },
        false => Boxed { values: [16, 17, 18] },
    }).values {
        total = total + item
    }
    return total
}
