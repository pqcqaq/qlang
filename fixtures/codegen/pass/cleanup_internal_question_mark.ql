extern "c" fn first() -> Int

fn helper() -> Int {
    return first()
}

fn main() -> Int {
    defer helper()?
    return 0
}
