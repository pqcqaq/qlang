async unsafe fn helper(value: Int) -> Int {
    return value + 2
}

async unsafe fn main() -> Int {
    return await helper(5)
}
