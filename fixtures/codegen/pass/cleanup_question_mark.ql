extern "c" fn first()

fn helper() -> Int {
    return 1
}

fn main() -> Int {
    defer first()
    return helper()?
}
