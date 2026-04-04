fn main() -> Int {
    let run: (Int) -> Int = (value) => value + 1
    let alias = run
    return alias(41)
}
