extern "c" fn sink(value: Int)

async fn ready() -> Bool {
    return true
}

async fn check(value: Int) -> Bool {
    return value == 1
}

async fn main() -> Int {
    defer if await ready() {
        sink(1);
    }
    defer match true {
        true if await check(1) => sink(2),
        _ => sink(3),
    }
    return 0
}
