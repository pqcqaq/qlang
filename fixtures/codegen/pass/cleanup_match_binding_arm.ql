extern "c" fn sink(value: Int)
extern "c" fn fallback()

fn main() -> Int {
    let value = 42
    defer match value {
        current if current == 42 => sink(current),
        _ => fallback(),
    }
    return 0
}
