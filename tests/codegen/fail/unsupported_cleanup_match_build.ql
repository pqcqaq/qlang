extern "c" fn first()

fn enabled() -> Bool {
    return false
}

fn main() -> Int {
    let flag = true
    defer first()
    return match flag {
        true if enabled() => 1,
        false => 0,
    }
}
