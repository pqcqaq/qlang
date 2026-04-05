fn main() -> Int {
    let first = 1
    let second = 2
    let capture_first = () => first
    let capture_second = () => second
    var alias = capture_first
    alias = capture_second
    return alias()
}
