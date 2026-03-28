extern "c" fn first()

fn main() -> Int {
    defer first()
    let capture = () => 1
    return 0
}
