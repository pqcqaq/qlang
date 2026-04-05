extern "c" fn first()

fn main() -> Int {
    defer first()
    let value = 1
    let capture = () => value
    let alias = capture
    return alias()
}
