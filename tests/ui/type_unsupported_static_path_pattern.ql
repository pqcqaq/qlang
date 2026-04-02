use TOTAL as Count

static TOTAL: Int = 2

fn main(value: Int) -> Int {
    match value {
        TOTAL => 1,
        Count => 2,
        _ => 0,
    }
}
