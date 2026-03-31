struct Wrap {
    values: [Int; 0],
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

fn score(values: [Int; 0], value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await empty_values()
    let second = await wrapped()
    return score(first, second)
}
