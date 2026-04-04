const CHECK: (Int) -> Bool = (value: Int) => value == 42
static APPLY: (Int) -> Int = (value: Int) => value + 1

fn main() -> Int {
    defer APPLY(41)
    return match 42 {
        current if CHECK(current) => 1,
        _ => 0,
    }
}
