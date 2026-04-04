fn running() -> Bool {
    return false
}

fn step() {
    return
}

fn main() -> Int {
    defer {
        while running() {
            step()
        }
    }
    return 0
}
