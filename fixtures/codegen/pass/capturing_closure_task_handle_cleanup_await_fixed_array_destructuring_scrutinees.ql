extern "c" fn sink(value: Int)

async fn load_values(value: Int) -> [Int; 3] {
    return [value, value + 1, value + 2]
}

async fn main() -> Int {
    let branch = true
    let left_task = spawn load_values(30)
    let right_task = spawn load_values(2)

    let left = () => left_task
    let right = () => right_task

    defer {
        sink(match await (if branch { left } else { right })() {
            [first, _, last] => first + last,
        });
    }

    defer match await (match branch { true => left, false => right })() {
        [first, middle, last] if first == 30 => sink(first + middle + last),
        _ => sink(0),
    }
    return 0
}
