fn main() -> Int {
    let base = 41
    let capture = () => base + 1
    let alias = capture
    return alias()
}
