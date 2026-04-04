use APPLY as run

fn add_one(value: Int) -> Int {
    return value + 1
}

const APPLY: (Int) -> Int = add_one

fn main() -> Int {
    defer run(41)
    return 0
}
