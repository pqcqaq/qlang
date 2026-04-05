fn main() -> Int {
    let value = 1
    let capture = () => value
    let alias = capture
    return alias()
}
