extern "c" fn first()
extern "c" fn second()

fn main() -> Int {
    defer {
        first();
        second()
    }
    return 0
}
