fn main() -> Int {
    let base = 41
    let capture = () => base + 1
    var alias = capture
    alias = capture
    return alias()
}
