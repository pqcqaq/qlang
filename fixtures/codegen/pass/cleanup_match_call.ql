extern "c" fn first()
extern "c" fn second()

fn enabled() -> Bool {
    return true
}

fn main() -> Int {
    let index = 1
    defer match index {
        0 => first(),
        1 if enabled() => second(),
        _ => first(),
    }
    return 0
}
