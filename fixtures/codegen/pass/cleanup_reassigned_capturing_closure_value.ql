extern "c" fn first()

fn main() -> Int {
    defer first()
    let base = 1
    let next = 2
    let capture_base = () => base
    let capture_next = () => next
    var alias = capture_base
    alias = capture_next
    return alias()
}
