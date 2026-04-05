fn main() -> Int {
    let value = 1
    let capture = () => value
    var alias = capture
    return alias()
}
